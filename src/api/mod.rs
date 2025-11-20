mod files;
mod oauth;

// OAuth scopes required for Sheets API access
// Note: drive.readonly is required because google_sheets4 crate uses it as the default scope
// for API calls like spreadsheets().get(). We also include spreadsheets for full read/write access.
const OAUTH_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/spreadsheets",
    "https://www.googleapis.com/auth/drive.readonly",
];

pub(crate) use oauth::TokenProvider;
