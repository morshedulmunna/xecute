---
title: "Multipart uploads"
description: "Validate streaming uploads and use private local or optional S3 storage."
---

Use this guide for multipart file requests. It needs the upload feature and explicit storage configuration; ordinary JSON endpoints do not need it.

A runnable controller example is included at
[`xecute/examples/file_upload.rs`](https://github.com/morshedulmunna/xecute/blob/main/xecute/examples/file_upload.rs).
Run from the repository root:

```sh
cargo run -p xecute --example file_upload --features upload
curl -F 'file=@report.pdf;type=application/pdf' http://127.0.0.1:3000/uploads
curl -F 'file=@photo.png;type=image/png' -F 'file=@report.pdf;type=application/pdf' \
  http://127.0.0.1:3000/uploads
```

The controller returns HTTP 201 with an array of `key`, `original_name`,
`content_type`, `size`, and `sha256`. It accepts PNG/JPEG/PDF content, at most four
files, 1 MiB per file, and 2 MiB total; HTTP framing has an additional 64 KiB
allowance. MIME signatures must match declared types. Files are private under
`target/file-upload-example/objects/`, with temporary files in `staging/`.
`/openapi.json` describes the multipart endpoint; the example has no download route.
Invalid multipart requests return 400, oversized uploads 413, invalid types 415,
and missing/empty/excess files 422, with a consistent error envelope and request ID.

This standalone example binds loopback and is separate from generated projects.
Before deploying it, add authentication and application rate/time limits, use a
persistent private storage directory, and define cleanup/retention. Content
signature validation is not malware scanning. Batch rollback is best effort;
crashes or cancellation can leave objects requiring reconciliation.


New projects do not include a built-in uploads module or upload configuration.
To implement uploads in your own feature module, enable the runtime `upload`
feature (and `upload-s3` for S3). The runtime provides `UploadPolicy`,
`UploadService`, `LocalStorage`, `UploadStorage`, and optional `S3Storage`.
Use `#[multipart]` on the upload handler for multipart OpenAPI metadata.

```toml
[dependencies]
xecute = { version = "0.1.8", features = ["upload"] }
```

Apply HTTP body limits and authentication to your own upload route, keep storage
outside `public/`, and configure its policy and backend explicitly.

## Keep learning

[Static files](/docs/http/static-files). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#multipart-file-uploads).
