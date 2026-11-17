# Privacy Policy for eBIRForms (macOS)

**Last updated:** June 13, 2026

Your privacy is important to us. This Privacy Policy explains how eBIRForms (the "App") handles your data.

## 1. Data Collection and Usage
eBIRForms is a local-first application designed to help you prepare and file BIR forms. Goldcoders does not operate an intermediary server that receives your taxpayer records. Data leaves the device only when you initiate or enable a feature that communicates directly with the selected external service.

## 2. Local Storage
All data entered into the App, including tax profiles, journal entries, generated tax forms, and settings, is stored locally on your device within an encrypted SQLite database in the application support directory.

The App communicates directly with official government endpoints, including BIR and eFPS endpoints, when you initiate a tax filing or related government transaction. Goldcoders does not intercept or proxy that traffic.

## 3. Third-Party Services
The App does not include third-party analytics, advertising SDKs, or behavioral trackers.

Optional features communicate directly with Google:

- **Google Calendar:** After you connect a Google account and create a profile calendar, the App sends the profile display name, masked TIN suffix, applicable form numbers, filing periods, filing status, and calculated deadlines to Google Calendar. Google stores these calendar events and delivers reminders through its services.
- **Gmail receipt tracking:** When enabled, the App uses Google OAuth and Gmail IMAP access to inspect the connected mailbox for BIR filing receipts. OAuth tokens are stored in the operating-system credential store.
- **Gemini COR extraction:** When you explicitly enable BYOK Gemini OCR for a COR upload, the selected document is sent directly to Google Gemini using your API key for extraction. The App does not send the document to Goldcoders.

Google processes this information under the terms and privacy policy associated with your Google account. Disconnecting an account removes locally stored credentials but does not automatically delete information already stored by Google. Profile calendars can be deleted explicitly from the profile Calendar tab.

## 4. Contact Us
If you have any questions or concerns about this privacy policy, please open an issue on our official GitHub repository.
