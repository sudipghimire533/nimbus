//TODO License

//! Block executive to be used by relay chain validators when validating parachain blocks built
//! with the nimubs consensus family.

use frame_support::traits::ExecuteBlock;
use sp_api::{BlockT, HeaderT};
//TODO don't leave it this way
use log::info as debug;
use sp_runtime::{RuntimeAppPublic, generic::DigestItem};
use nimbus_primitives::{NimbusId, NimbusSignature};
use sp_application_crypto::{TryFrom, Public as _};

/// Block executive to be used by relay chain validators when validating parachain blocks built
/// with the nimubs consensus family.
///
/// This will strip the seal digest, and confirm that only a single such digest exists.
/// It then passes the pre-block to the inner executive which will likely be the normal FRAME
/// executive as it is run on the parachain itself.
/// (Aspitational) Finally it puts the original digest back on and confirms the blocks match
///
/// Essentially this contains the logic of the verifier and the normal executive.
/// TODO Degisn improvement:
/// Can we share code with the verifier?
/// Can this struct take a verifier as an associated type?
/// Or maybe this will just get simpler in general when https://github.com/paritytech/polkadot/issues/2888 lands
pub struct BlockExecutor<T, I>(sp_std::marker::PhantomData<(T, I)>);

impl<Block, T, I> ExecuteBlock<Block> for BlockExecutor<T, I>
where
	Block: BlockT,
	I: ExecuteBlock<Block>,
{
	fn execute_block(block: Block) {
		let (mut header, extrinsics) = block.deconstruct();

		debug!(target: "executive", "In hacked Executive. Initial digests are {:?}", header.digest());

		// Set the seal aside for checking.
		let seal = header
			.digest_mut()
			.logs
			.pop()
			.expect("Seal digest is present and is last item");

		debug!(target: "executive", "In hacked Executive. digests after stripping {:?}", header.digest());
		debug!(target: "executive", "The seal we got {:?}", seal);

		let sig = match seal {
			DigestItem::Seal(id, ref sig) if id == *b"nmbs" => sig.clone(),
			// Seems I can't return an error here, so I guess I have to panic
			_ => panic!("HeaderUnsealed"),
		};

		debug!(target: "executive", "🪲 Header hash after popping digest {:?}", header.hash());

		debug!(target: "executive", "🪲 Signature according to executive is {:?}", sig);

		// Grab the digest from the runtime
		//TODO use the CompatibleDigest trait. Maybe this code should move to the trait.
		let consensus_digest = header
			.digest()
			.logs
			.iter()
			.find(|digest| {
				match *digest {
					DigestItem::Consensus(id, _) if id == b"nmbs" => true,
					_ => false,
				}
			})
			.expect("A single consensus digest should be added by the runtime when executing the author inherent.");
		
		let claimed_author = match *consensus_digest {
			DigestItem::Consensus(id, ref author_id) if id == *b"nmbs" => author_id.clone(),
			_ => panic!("Expected consensus digest to contains author id bytes"),
		};

		debug!(target: "executive", "🪲 Claimed Author according to executive is {:?}", claimed_author);

		// Verify the signature

		// Is this gonna work? I'm not sure I have access to the NimbusPair in wasm.
		// This is copied from  my keystore learning. It may have to work differently in wasm. Basti used RuntimeAppPublic (I think)
		// to do this check in aura.
		// let valid_signature = NimbusPair::verify(
		// 	&NimbusSignature::try_from(sig).expect("Bytes should convert to signature correctly"),
		// 	header.hash(),
		// 	&NimbusId::from_slice(&claimed_author),
		// );

		let valid_signature = NimbusId::from_slice(&claimed_author).verify(
			&header.hash(),
			&NimbusSignature::try_from(sig).expect("Bytes should convert to signature correctly"),
		);

		debug!(target: "executive", "🪲 Valid signature? {:?}", valid_signature);

		if !valid_signature{
			panic!("Block signature invalid");
		}
		

		// Now that we've verified the signature, hand execution off to the inner executor
		// which is probably the normal frame executive.
		I::execute_block(Block::new(header, extrinsics));
	}
}
