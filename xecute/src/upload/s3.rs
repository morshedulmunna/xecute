use super::{
    S3Options, StorageFuture, StoredFile, UploadError, UploadStorage, ValidatedUpload, valid_key,
};
use aws_sdk_s3::{Client, config::Region, primitives::ByteStream};

/// Official AWS SDK adapter. Bodies stream from validated staging files and remain retryable.
#[derive(Clone)]
pub struct S3Storage {
    client: Client,
    bucket: String,
    prefix: String,
}
impl S3Storage {
    /// Use an application-supplied AWS client (including custom transports and credential providers).
    pub fn new(client: Client, bucket: &str, prefix: &str) -> Result<Self, UploadError> {
        let options = S3Options {
            bucket: bucket.into(),
            region: "us-east-1".into(),
            endpoint: None,
            force_path_style: false,
            allow_http: false,
            prefix: prefix.into(),
        };
        options.validate()?;
        Ok(Self {
            client,
            bucket: bucket.into(),
            prefix: prefix.into(),
        })
    }
    /// Build a client using the standard refreshable AWS credential chain, including
    /// environment credentials, profiles, and workload roles. Explicit region/endpoint
    /// settings come from the supplied options. No secret is included in diagnostics.
    pub async fn from_environment(options: S3Options) -> Result<Self, UploadError> {
        options.validate()?;
        let shared = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(Region::new(options.region.clone()))
            .load()
            .await;
        let mut builder =
            aws_sdk_s3::config::Builder::from(&shared).force_path_style(options.force_path_style);
        builder.set_endpoint_url(options.endpoint);
        let client = Client::from_conf(builder.build());
        Self::new(client, &options.bucket, &options.prefix)
    }
    fn object_key(&self, key: &str) -> Result<String, UploadError> {
        if !valid_key(key) {
            return Err(UploadError::InvalidFilename);
        }
        Ok(if self.prefix.is_empty() {
            key.into()
        } else {
            format!("{}/{key}", self.prefix)
        })
    }
}
impl UploadStorage for S3Storage {
    fn put<'a>(&'a self, upload: &'a ValidatedUpload) -> StorageFuture<'a, StoredFile> {
        Box::pin(async move {
            let key = self.object_key(&upload.metadata().key)?;
            let body = ByteStream::from_path(upload.path())
                .await
                .map_err(|_| UploadError::Storage)?;
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(key)
                .body(body)
                .content_type(&upload.metadata().content_type)
                .content_length(upload.metadata().size as i64)
                .metadata("sha256", &upload.metadata().sha256)
                .if_none_match("*")
                .send()
                .await
                .map_err(|_| UploadError::Storage)?;
            Ok(upload.metadata().clone())
        })
    }
    fn delete<'a>(&'a self, key: &'a str) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let key = self.object_key(key)?;
            self.client
                .delete_object()
                .bucket(&self.bucket)
                .key(key)
                .send()
                .await
                .map_err(|_| UploadError::Storage)?;
            Ok(())
        })
    }
}
