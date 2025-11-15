# Setting Up Google Sheet Access

Follow these instructions to set up Google Sheets access.

### Step 1: Create Google Cloud Project

Tiller Sync requires OAuth credentials to access your Google Sheets. You'll need to create these
through the Google Cloud Console.

1. Navigate to the [Google Cloud Console](https://console.cloud.google.com/)
2. Click the project dropdown at the top of the page
3. Click **"New Project"** in the dialog that appears
4. Enter a project name (e.g., "Tiller Sync")
5. Click **"Create"**

![setup-01-create-project](docs/images/setup-01-create-project.jpg)

**Wait for the project to be created** (this may take a few seconds). Once created, ensure you've
selected your new project from the project dropdown.

### Step 2: Enable Google Sheets API

1. In the Google Cloud Console, ensure your "Tiller Sync" project is selected
2. Navigate to **"APIs & Services"** > **"Library"** (use the left sidebar or search)  
   <img src="docs/images/setup-02a-enable-api.jpg" alt="drawing" width="300"/>
3. In the API Library search box, type **"Google Sheets API"**  
   <img src="docs/images/setup-02b-enable-api.jpg" alt="drawing" width="300"/>
4. Click on **"Google Sheets API"** in the results
5. Click the **"Enable"** button  
   <img src="docs/images/setup-02c-enable-api.jpg" alt="drawing" width="300"/>

### Step 3: Configure OAuth Consent Screen

Before creating credentials, you must configure the OAuth consent screen.

1. Navigate to **"APIs & Services"** > **"OAuth consent screen"**  
   <img src="docs/images/setup-03a-consent-type.jpg" alt="drawing" width="300"/>
2. Click "Get Started"  
   <img src="docs/images/setup-03b-consent-type.jpg" alt="drawing" width="300"/>
5. Fill in the required fields on the "OAuth consent screen" page:
    - **App name**: `Tiller Sync` (or your preferred name)  
      <img src="docs/images/setup-03c-consent-type.jpg" alt="drawing" width="300"/>
    - **User support email**: Select your email from the dropdown
    - Select **"External"** as the user type (unless you have a Google Workspace account and want to
      restrict to your organization)  
      <img src="docs/images/setup-03d-consent-type.jpg" alt="drawing" width="300"/>
    - **Developer contact information**: Enter your email address
6. Leave other fields with their default values

### Step 4: Configure Data Access

1. On the **"Data Access"** page, click **"Add or Remove Scopes"**  
   <img src="docs/images/setup-03e-data-access.jpg" alt="drawing" width="300"/>
2. In the filter box, search for `sheets`  
   <img src="docs/images/setup-03f-search-sheets.jpg" alt="drawing" width="300"/>
3. Select the checkbox for **`https://www.googleapis.com/auth/spreadsheets`**
    - This scope allows read and write access to Google Sheets  
      <img src="docs/images/setup-03g-spreadsheet-scope.jpg" alt="drawing" width="300"/>
4. Click **"Update"** at the bottom of the dialog, then "Save"

### Step 5: Create a User

1. Navigate to the **Audience** tab and scroll down to **Test users**:  
   <img src="docs/images/setup-05a-audience-tab.jpg" alt="drawing" width="300"/>

2. Add your same email here as the **Test user** and Save:  
   <img src="docs/images/setup-05b-test-user.jpg" alt="drawing" width="300"/>

### Step 6: Create OAuth Credentials

1. Navigate to **"APIs & Services"** > **"Credentials"**  
   <img src="docs/images/setup-06a-credentials.jpg" alt="drawing" width="300" border="1"/>
2. Click **"+ Create Credentials"** at the top
3. Select **"OAuth client ID"** from the dropdown  
   <img src="docs/images/setup-06b-oauth-client-id.jpg" alt="drawing" width="300" border="1"/>
4. For **Application type**, select **"Desktop app"**
5. Enter a name: `Tiller Sync` (or your preferred name)  
   <img src="docs/images/setup-06c-desktop-app-name.jpg" alt="drawing" width="300" border="1"/>
6. Click **"Create"**
7. A dialog will appear showing your **Client ID** and **Client Secret**
    - Download the JSON file and rename it to api_key.json  
      <img src="docs/images/setup-06d-creds-dialog.jpg" alt="drawing" width="300" border="1"/>
8. Click **"OK"** to close the dialog

### Step 7: Move the Downloaded File

1. In the previous step you downloaded a JSON file, if you didn't rename it, do so now:
    - `client_secret_xxxxx.apps.googleusercontent.com.json` -> `api_key.json`
2. **Move the file** to your Tiller home directory:
   ```bash
   # Create the Tiller directories
   mkdir -p ~/tiller/.secrets

   # Move the downloaded file (adjust the source path to your Downloads folder)
   mv ~/Downloads/api_key.json ~/tiller/.secrets/api_key.json

   # Set restrictive permissions
   chmod 600 ~/tiller/.secrets/api_key.json
   ```

### Step 8: Authenticate Tiller Sync

Now that you have your OAuth credentials set up, you can authenticate Tiller Sync:

1. Run the authentication command:
   ```bash
   tiller auth
   ```

2. The command will:
    - Automatically open your web browser to Google's authorization page
    - If the browser doesn't open automatically, copy the URL displayed in the terminal

3. In the browser:
    - Select the Google account you use for Tiller
    - You may see a warning that "Google hasn't verified this app"
        - Click **"Advanced"**
        - Click **"Go to Tiller Sync (unsafe)"**
    - Review the permissions requested
    - Click **"Allow"**

> **Screenshot needed**: `docs/images/setup-10-oauth-consent.png`
>
> Show the Google OAuth consent screen with the "Allow" button visible.

4. After clicking "Allow", you should see a success message in your browser
5. Return to your terminal - you should see:
   ```
   ✓ Authorization successful!
   ✓ Tokens saved to: /Users/you/tiller/.secrets/token.json
   ```

6. Verify your authentication:
   ```bash
   tiller auth verify
   ```

   You should see:
   ```
   ✓ Authentication verified successfully
     Spreadsheet: Tiller Foundation Template
     Access: Read/Write
   ```

### You're Done the Hard Part 😅

Return to the [readme](../README.md) for the fun parts.
