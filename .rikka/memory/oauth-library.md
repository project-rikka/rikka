---
title: Shared OAuth client library (crates/foundation/rikka-oauth)
description: rikka-oauth owns loopback redirects, PKCE, code/refresh/device grants, tokens, claims, discovery and renewal.
about: crates/foundation/rikka-oauth
tags: [oauth, auth]
---

- `crates/foundation/rikka-oauth` is one public crate over `rikka-net` only. Every OAuth sign-in in Rikka goes through it; apps depend on it directly.
- `Client::builder(id, token_endpoint)` holds the registration: `.secret()` (client_secret_post, or client_secret_basic after `.basic()`), `.json()` for JSON token bodies, `.header()`; `Accept: application/json` is the default. `client.http()` is its 30 s-timeout HTTP client for profile calls.
- Browser flow: `Loopback::localhost(port)` or `::ip(port)` (`127.0.0.1`), `.path()`, `.bind()` → `Redirect` with `uri()`; `client.authorization(endpoint, redirect.uri()).scope().pkce().param().build()` → `Authorization { url, state }`; `redirect.code(state)` ignores stray callbacks (`redirect.callback(state)` hands back the whole `Callback`, for `iss`); `client.exchange(code, &authorization)` adds redirect_uri and verifier.
- Non-standard callbacks (OpenRouter, Qobuz) use `redirect.wait()` → `Callback::get`/`code()`, and `Pkce` directly.
- `client.refresh(refresh_token)` keeps that refresh token when none is returned. `client.device(endpoint).scope().send()` → `Device`, whose `token()` polls RFC 8628.
- `Token` exposes access/refresh tokens, `expires_at`, `identity()` (`Claims` of the id_token) and the raw `body()`. `Claims::decode(jwt)` decodes without verifying.
- `Token` is serde-storable as `{access_token, refresh_token?, id_token?, expires_at}`; `body()` is `Null` once read back, and reading refuses an empty access token.
- `rikka_oauth::send(request)` reads JSON and turns a failure into `Error::ServerResponse(ErrorResponse)` (RFC 6749 §5.2); `ErrorResponse::is_expired()` means invalid_grant/refresh_token_reused.
- Discovery: `Metadata::discover(url)` reads one OIDC/RFC 8414 document; `Metadata::discover_issuer(issuer)` tries the RFC 8414 and OpenID well-known locations in order, returns `None` when none exists and refuses a document naming another issuer. `Metadata` exposes endpoints, `scopes_supported`, `supports_s256()`, `supports_secret_basic()` and `check_response_issuer(iss)` (RFC 9207). `ProtectedResource::discover(resource)`/`fetch(metadata_url, resource)` read RFC 9728 metadata and refuse one that describes another resource. `Challenge::parse(www_authenticate)` reads `resource_metadata`, `scope` and `error`. `Registration::new(name).redirect_uri(..).scope(..).send(endpoint)` registers a public native client (RFC 7591) → `Registered`. RFC 8707 `resource` is passed with `.param("resource", ..)` and `.field("resource", ..)`.
- `RefreshingToken<T: Refresh>` refreshes 5 min before expiry (or once, when expiry is unknown), single-flight, with an `on_refresh` callback. `Token::fill_missing_from(previous)` keeps the old refresh and id tokens when a refresh response omits them.
- `rikka-llm` converts `rikka_oauth::Error` into its own errors; `rikka-llm-adapter` keeps only `Credentials` (headers) and `renewing()`, which wraps `RefreshingToken` as a `Transport` authorizer.
