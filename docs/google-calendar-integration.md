# Google Calendar Integration Setup

eBIRForms can create one Google secondary calendar for each tax profile. Each
calendar appears separately in Google Calendar, so users managing multiple TINs
can show or hide each profile independently.

The integration publishes only dated obligations from the profile's
authoritative yearly Forms Sets. Google sends email reminders seven days and one
day before each filing deadline.

## Google Cloud Configuration

1. Open the [Google Cloud Console](https://console.cloud.google.com/) and create
   or select the project used by eBIRForms.
2. Open **APIs & Services > Library**, find **Google Calendar API**, and enable
   it.
3. Open **Google Auth Platform** and configure the consent screen:
   - Choose **Internal** when the app will be used only by accounts in one
     Google Workspace organization.
   - Choose **External** when personal Gmail accounts or accounts outside the
     organization must connect.
   - While the app is in Testing mode, add every account that will test the
     integration under **Test users**.
4. Add the scope:

   ```text
   https://www.googleapis.com/auth/calendar.app.created
   ```

   eBIRForms also requests `userinfo.email` to display the connected account.
   The Calendar scope permits the app to manage only calendars it created; it
   does not grant access to the user's existing calendars.
5. Open **Clients**, create an OAuth client, and select **Desktop app** as the
   application type.
6. Record the generated client ID and client secret.

## Build Configuration

Set these variables in `.env` for local development or in the environment used
to build a release:

```dotenv
GOOGLE_CLIENT_ID=your-desktop-client-id.apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=your-desktop-client-secret
```

The values are compiled into release builds through `option_env!`, so rebuild
the application after changing them. Desktop OAuth client secrets cannot be
treated as confidential; the requested least-privilege scope is the security
boundary.

For GitHub tag releases, create repository Actions secrets named
`GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET`. The release workflow exposes
those values only to the build process. App Store builds made with
`just package-mac-appstore` load the same names from the shell or project
`.env`.

Do not store user access or refresh tokens in `.env`. eBIRForms stores the
Calendar token bundle in the operating-system credential store. On macOS this
is the login Keychain.

## Connect and Create Calendars

1. Open **eBIRForms > Settings > Google Calendar**.
2. Select **Connect Google** and authorize the account in the browser.
3. Edit an existing tax profile and open its **Calendar** tab.
4. Confirm the calendar name and select **Create Google Calendar**.
5. Repeat for every tax profile that should have a separate calendar.
6. Open Google Calendar. Each profile calendar appears separately in the left
   sidebar and can be checked or unchecked independently.

The application synchronizes:

- after profile, Forms Set, deadline, or filing-state changes;
- every six hours while eBIRForms is running; and
- whenever **Sync Now** is selected.

Google continues delivering event reminders even when eBIRForms is closed.

## Synchronization Behavior

- All configured Forms Set years are synchronized.
- Deadline changes update the existing Google event instead of creating a
  duplicate.
- Removing a form from a Forms Set removes its managed events.
- Paid filings remain visible with a `[Filed]` prefix and no future reminders.
- Archiving a profile removes its managed events on the next sync. Restoring it
  recreates the applicable events.
- Forms without a calculable date, such as event-based obligations, are reported
  as excluded.
- eBIRForms owns managed event fields. Manual edits to those fields may be
  replaced on the next synchronization.
- **Unlink** preserves the remote Google calendar.
- **Delete Calendar** permanently deletes the remote calendar and its events.
- Disconnecting the global account preserves all remote calendars.
- A profile cannot be permanently deleted while a Google calendar remains
  linked. Delete the remote calendar or explicitly unlink it first.

## Troubleshooting

### Google OAuth is not configured

The build does not contain `GOOGLE_CLIENT_ID` or `GOOGLE_CLIENT_SECRET`. Set both
values and rebuild the application.

### Access blocked or user is not authorized

For an External app in Testing mode, add the Google account as a test user. For
an Internal app, sign in with an account in the configured Workspace
organization.

### Google Calendar API error 403

Confirm that Google Calendar API is enabled in the same Cloud project as the
OAuth client. A Workspace administrator may also need to allow the application
or its OAuth scope.

### Missing refresh token

Reconnect the account from Settings. eBIRForms requests offline access and
forces the consent screen so Google returns a refresh token.

### Calendar was deleted in Google

The profile Calendar tab reports that the remote resource was not found.
Unlink the missing calendar, then create it again.

### Authorization was revoked

Reconnect the Google account in Settings, then use **Sync Now** on each linked
profile.

## References

- [Google Calendar API authorization](https://developers.google.com/workspace/calendar/api/auth)
- [Create secondary calendars](https://developers.google.com/workspace/calendar/api/v3/reference/calendars/insert)
- [Create events](https://developers.google.com/workspace/calendar/api/guides/create-events)
- [OAuth for desktop applications](https://developers.google.com/identity/protocols/oauth2/native-app)
- [Configure OAuth consent](https://developers.google.com/workspace/guides/configure-oauth-consent)
