use crate::api::files::{File, SecretFile, TokenFile};
use crate::api::OAUTH_SCOPES;
use crate::Result;
use anyhow::{anyhow, bail, Context};
use chrono::Utc;
use hyper::StatusCode;
use log::{debug, error};
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl,
    RefreshToken, Scope, TokenResponse, TokenUrl,
};
use std::fmt::Debug;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub(crate) struct TokenProvider {
    secret: File<SecretFile>,
    token: File<TokenFile>,
}

impl TokenProvider {
    /// Runs the complete OAuth workflow with user interaction to create a new file at `token`.
    ///
    /// # Sequence
    /// - Deserializes the file at `secret`
    /// - Deletes the file at `token` if it already exists
    /// - Runs a local webserver on a random port to receive the callback from the Google OAuth
    ///   workflow
    /// - Uses the data from `secret` to generate a URL for the user to paste into their browser.
    /// - When the user authorizes the proper scopes in the web-browser, our local webserver catches
    ///   the callback.
    /// - After catching the callback, we gracefully shut down the local server.
    /// - The token information is taken from the callback and serialized to the `token` filepath.
    /// - The file at `token` has its permissions set correctly if we are on Linux or Unix
    /// - We return a fully constructed `Self` that is ready to provide tokens and refresh them.
    ///
    /// # Arguments
    /// - `secret`: The path to an existing file that contains the client ID and client secret.
    /// - `token`: The path to a file that may or may not already exist. This is the path where we
    ///   will store our OAuth token, refresh token, and other token metadata such as expiration.
    ///
    /// # Returns
    /// - A constructed `TokenProvider` object.
    ///
    /// # Errors
    /// - If any of the file operations, network operations, or logical checks fail.
    pub(crate) async fn initialize<P1, P2>(secret: P1, token: P2) -> Result<Self>
    where
        P1: Into<PathBuf>,
        P2: Into<PathBuf>,
    {
        let secret_path = secret.into();
        let token_path = token.into();

        // Load the client secret file
        let secret_file = File::<SecretFile>::load(&secret_path).await?;

        // Delete existing token file if it exists
        if token_path.exists() {
            tokio::fs::remove_file(&token_path)
                .await
                .context("Failed to delete existing token file")?;
        }

        // Create OAuth client
        let oauth_client = create_oauth_client(secret_file.data())?;

        // Generate PKCE challenge
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Find an available port and start HTTP server
        let (listener, port) = bind_random_port()?;
        let redirect_url = format!("http://localhost:{port}");

        // Build authorization URL
        let redirect_uri_for_auth =
            RedirectUrl::new(redirect_url.clone()).context("Invalid redirect URL")?;
        let (auth_url, csrf_token) = oauth_client
            .authorize_url(CsrfToken::new_random)
            .add_scopes(OAUTH_SCOPES.iter().map(|s| Scope::new(s.to_string())))
            .set_pkce_challenge(pkce_challenge)
            .set_redirect_uri(std::borrow::Cow::Borrowed(&redirect_uri_for_auth))
            .url();

        // Print instructions to user
        println!("\nOpening browser for authorization...");
        println!("If browser doesn't open automatically, visit:");
        println!("\n{auth_url}");

        // Try to open browser
        let _ = open_browser(auth_url.as_ref());

        // Wait for OAuth callback
        let auth_code = receive_oauth_callback(listener, csrf_token).await?;

        // Exchange authorization code for token
        let redirect_uri = RedirectUrl::new(redirect_url).context("Invalid redirect URL")?;
        let token_response = oauth_client
            .exchange_code(AuthorizationCode::new(auth_code))
            .set_pkce_verifier(pkce_verifier)
            .set_redirect_uri(std::borrow::Cow::Owned(redirect_uri))
            .request_async(&async_http_client)
            .await
            .context("Failed to exchange authorization code for token")?;

        // Calculate expiration time
        let expires_in = token_response
            .expires_in()
            .unwrap_or(std::time::Duration::from_secs(3600));
        let expires_at = Utc::now() + chrono::Duration::from_std(expires_in)?;

        // Create token file
        let token_data = TokenFile::new(
            OAUTH_SCOPES.iter().map(|s| s.to_string()).collect(),
            token_response.access_token().secret().clone(),
            token_response
                .refresh_token()
                .context("No refresh token received")?
                .secret()
                .clone(),
            expires_at,
            None, // id_token not available in BasicTokenResponse
        );

        let token_file = File::new(token_path, token_data);
        token_file.save().await?;

        println!("✓ Authorization successful!");
        println!("✓ Tokens saved");

        Ok(Self {
            secret: secret_file,
            token: token_file,
        })
    }

    /// Constructs a `TokenProvider` where existing tokens exist.
    ///
    /// # Sequence
    /// - Deserializes the file at `secret`
    /// - Deserializes the file at `token`
    /// - Runs some validation checks on the `token` file to make sure we believe it will work: e.g.
    ///   - Expiration of the refresh token.
    ///   - Existence of the scopes we know we will need.
    /// - Returns a constructed `TokenProvider`
    ///
    /// DOES NOT interact with the user to obtain initial authentication.
    ///
    /// # Arguments
    /// - `secret`: The path to an existing file that contains the client ID and client secret.
    /// - `token`: The path to an existing file that contains the token and refresh token.
    ///
    /// # Returns
    /// - A constructed `TokenProvider` object.
    ///
    /// # Errors
    /// - If any of the file operations, network operations, or logical checks fail.
    pub(crate) async fn load<P1, P2>(secret: P1, token: P2) -> Result<Self>
    where
        P1: Into<PathBuf>,
        P2: Into<PathBuf>,
    {
        let secret_path = secret.into();
        let token_path = token.into();

        // Load both files
        let secret_file = File::<SecretFile>::load(&secret_path).await?;

        // Load and validate token file (validates that required scopes are present)
        let token_data = TokenFile::load(&token_path).await?;
        let token_file = File::new(&token_path, token_data);

        Ok(Self {
            secret: secret_file,
            token: token_file,
        })
    }

    /// Returns the current token without checking its expiration or refreshing it.
    pub(super) fn token(&self) -> &str {
        self.token.data().access_token()
    }

    /// - Checks to see if our token is expired or expiring soon (within `EXPIRATION` minutes)
    /// - If our token needs to be refreshed, does so with the `oauth` library. Note: DOES NOT use
    ///   user interaction, no matter what happens.
    /// - If an error occurs while attempting to refresh the token, the error is returned.
    /// - If the token refresh is successful, updates the state of `self` with it, and saves it to
    ///   our token file path.
    /// - Returns the `token` for the sheets client to use.
    pub(super) async fn token_with_refresh(&mut self) -> Result<&str> {
        // Check if token needs refresh
        if !self.token.data().is_expired() {
            return Ok(self.token.data().access_token());
        }

        debug!("Access token expired, refreshing...");
        self.refresh().await?;
        debug!("✓ Token refreshed successfully");
        Ok(self.token.data().access_token())
    }

    /// Forces a refresh whether or not the token is expired.
    pub(crate) async fn refresh(&mut self) -> Result<&str> {
        // Create OAuth client
        let oauth_client = create_oauth_client(self.secret.data())?;

        // Refresh the token
        let refresh_token = RefreshToken::new(self.token.data().refresh_token().to_string());
        let token_response = oauth_client
            .exchange_refresh_token(&refresh_token)
            .request_async(&async_http_client)
            .await
            .context("Failed to refresh access token")?;

        // Calculate new expiration time
        let expires_in = token_response
            .expires_in()
            .unwrap_or(std::time::Duration::from_secs(3600));
        let expires_at = Utc::now() + chrono::Duration::from_std(expires_in)?;

        // Update token data
        self.token.data_mut().update(
            token_response.access_token().secret().clone(),
            expires_at,
            token_response.refresh_token().map(|t| t.secret().clone()),
        );

        // Save updated token
        self.token.save().await?;
        Ok(self.token())
    }
}

/// Async HTTP client for OAuth2 requests using reqwest
async fn async_http_client(
    request: oauth2::HttpRequest,
) -> std::result::Result<
    oauth2::HttpResponse,
    oauth2::RequestTokenError<
        oauth2::reqwest::Error,
        oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
    >,
> {
    let client = ::reqwest::Client::new();

    let mut req_builder = client
        .request(request.method().clone(), request.uri().to_string())
        .body(request.body().to_vec());

    for (name, value) in request.headers() {
        req_builder = req_builder.header(name.as_str(), value.as_bytes());
    }

    let response = req_builder
        .send()
        .await
        .map_err(oauth2::RequestTokenError::Request)?;

    let status_code = response.status();
    let headers = response.headers().clone();
    let body = response
        .bytes()
        .await
        .map_err(oauth2::RequestTokenError::Request)?
        .to_vec();

    let mut builder = oauth2::http::Response::builder().status(status_code);

    for (name, value) in headers.iter() {
        builder = builder.header(name, value);
    }

    builder
        .body(body)
        .map_err(|e| oauth2::RequestTokenError::Other(format!("Failed to build response: {e}")))
}

/// OAuth Client type for Google OAuth
type GoogleOAuthClient = oauth2::Client<
    oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
    oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>,
    oauth2::StandardTokenIntrospectionResponse<
        oauth2::EmptyExtraTokenFields,
        oauth2::basic::BasicTokenType,
    >,
    oauth2::StandardRevocableToken,
    oauth2::StandardErrorResponse<oauth2::RevocationErrorResponseType>,
    oauth2::EndpointSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointSet,
>;

/// Create an OAuth client from the secret file
fn create_oauth_client(secret: &SecretFile) -> Result<GoogleOAuthClient> {
    let client_id = ClientId::new(secret.client_id().to_string());
    let client_secret = ClientSecret::new(secret.client_secret().to_string());
    let auth_url = AuthUrl::new(secret.auth_uri().to_string())
        .context("Invalid authorization endpoint URL")?;
    let token_url =
        TokenUrl::new(secret.token_uri().to_string()).context("Invalid token endpoint URL")?;

    Ok(BasicClient::new(client_id)
        .set_client_secret(client_secret)
        .set_auth_uri(auth_url)
        .set_token_uri(token_url))
}

/// Bind to a random available port
fn bind_random_port() -> Result<(TcpListener, u16)> {
    let listener = TcpListener::bind("127.0.0.1:0").context("Failed to bind to local address")?;
    let port = listener.local_addr()?.port();
    Ok((listener, port))
}

/// Attempt to open the authorization URL in the default browser
fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(&["/C", "start", url])
            .spawn()?;
    }
    Ok(())
}

/// Run HTTP server to receive OAuth callback
async fn receive_oauth_callback(listener: TcpListener, expected_csrf: CsrfToken) -> Result<String> {
    use hyper::server::conn::http1;
    use hyper::service::service_fn;
    use hyper::{Request, Response};
    use hyper_util::rt::TokioIo;

    // Wrap the auth code in Arc<Mutex> so we can share it between handler invocations
    let auth_code_result: Arc<Mutex<Option<Result<String>>>> = Arc::new(Mutex::new(None));

    let auth_code_clone = auth_code_result.clone();
    let expected_csrf_clone = expected_csrf.clone();

    // Create service
    let make_service = move |req: Request<hyper::body::Incoming>| {
        let auth_code = auth_code_clone.clone();
        let csrf = expected_csrf_clone.clone();

        async move {
            let uri = req.uri();
            let query = uri.query().unwrap_or("");

            // Parse query parameters
            let params: std::collections::HashMap<String, String> =
                url::form_urlencoded::parse(query.as_bytes())
                    .into_owned()
                    .collect();

            let response = if let Some(code) = params.get("code") {
                // Verify CSRF token
                if let Some(state) = params.get("state") {
                    if state == csrf.secret() {
                        *auth_code.lock().await = Some(Ok(code.clone()));
                        Response::builder()
                            .status(StatusCode::OK)
                            .body(
                                "Authorization successful! You can close this window.".to_string(),
                            )
                            .unwrap()
                    } else {
                        *auth_code.lock().await = Some(Err(anyhow!("CSRF token mismatch")));
                        Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body("CSRF token mismatch".to_string())
                            .unwrap()
                    }
                } else {
                    *auth_code.lock().await = Some(Err(anyhow!("No state parameter")));
                    Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body("No state parameter".to_string())
                        .unwrap()
                }
            } else if let Some(error) = params.get("error") {
                let error_desc = params
                    .get("error_description")
                    .map(|s| s.as_str())
                    .unwrap_or("Unknown error");
                *auth_code.lock().await = Some(Err(anyhow!("OAuth error: {error} - {error_desc}")));
                Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(format!("Authorization failed: {error_desc}"))
                    .unwrap()
            } else {
                Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body("Missing authorization code".to_string())
                    .unwrap()
            };

            Ok::<_, anyhow::Error>(response)
        }
    };

    // Set non-blocking mode for the listener
    listener.set_nonblocking(true)?;

    // Convert std TcpListener to tokio TcpListener
    let tokio_listener = tokio::net::TcpListener::from_std(listener)?;

    println!("\nWaiting for authorization callback...\n");

    // Accept one connection
    let (stream, _) = tokio_listener.accept().await?;
    let io = TokioIo::new(stream);

    // Serve the connection
    tokio::task::spawn(async move {
        if let Err(err) = http1::Builder::new()
            .serve_connection(io, service_fn(make_service))
            .await
        {
            error!("Error serving connection: {err:?}");
            panic!("We cannot proceed because we could not start the local server: {err:?}");
        }
    });

    // Wait a moment for the request to be processed
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Check if we got the auth code
    let result = auth_code_result.lock().await.take();
    match result {
        Some(Ok(code)) => Ok(code),
        Some(Err(e)) => Err(e),
        None => bail!("Failed to receive authorization code"),
    }
}
