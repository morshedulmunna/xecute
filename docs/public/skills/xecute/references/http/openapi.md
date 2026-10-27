Use Swagger to try endpoints and OpenAPI to describe their contract. Start with the generated /docs/ endpoint, then add metadata when you add routes or change DTOs.

Every generated project includes embedded Swagger UI at `/docs/` and an OpenAPI
3.1 document at `/docs/openapi.json`. Resource generation adds each controller's
operations automatically. Set `docs.enabled = false` in `config/AppConfig.toml` to disable both routes.

| Annotation | Purpose |
| --- | --- |
| `#[api(summary = "...", tag = "...", operation_id = "...")]` | Describe an endpoint and its operation ID |
| `#[body(json = RequestDto)]` | Declare the JSON body type; it must match the `#[body]` parameter |
| `#[resp(201, "Created", json = ResponseDto)]` | Describe a response and schema |
| `#[resp(422, "Invalid payload")]` | Describe a response without a JSON schema |

DTOs and response entities derive `schemars::JsonSchema`. Keep schema constraints
such as `#[schemars(length(...))]` aligned with validation attributes. Schema
generation describes data; it does not itself validate requests. API metadata
does not change the HTTP status returned by a handler.

For manual documentation assembly, use `Controller::api_operations()` with
`xecute::openapi_document(title, version, operations)`.

## Keep learning

[Errors and domain mappings](errors.md). Return to the [learning path](../index.md) whenever you need a different topic.