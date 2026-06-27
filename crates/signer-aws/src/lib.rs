//! AWS KMS signer for the tronz TRON SDK.
//!
//! Delegates signing to AWS Key Management Service so that the private key
//! never leaves the HSM.  The key must be an **ECC_SECG_P256K1** asymmetric
//! signing key created in KMS.
//!
//! # Example
//!
//! ```no_run
//! use aws_config::BehaviorVersion;
//! use tronz_signer::TronSigner;
//! use tronz_signer_aws::AwsSigner;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
//! let client = aws_sdk_kms::Client::new(&config);
//!
//! let signer = AwsSigner::new(client, "your-key-id".to_string()).await?;
//! println!("TRON address: {}", signer.address());
//! # Ok(())
//! # }
//! ```

mod signer;

pub use signer::{AwsSigner, AwsSignerError};
