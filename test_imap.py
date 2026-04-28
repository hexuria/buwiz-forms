import imaplib

def test_app_password():
    email = "codeitlikemiley@gmail.com"
    password = "tnlriyqzolarsimf" # without spaces, or we can try with spaces
    
    try:
        print("Testing with spaces removed...")
        mail = imaplib.IMAP4_SSL("imap.gmail.com")
        mail.login(email, password)
        print("SUCCESS! Login worked with spaces removed.")
        mail.logout()
    except Exception as e:
        print(f"FAILED with spaces removed: {e}")
        
    try:
        print("\nTesting with original spaces...")
        mail = imaplib.IMAP4_SSL("imap.gmail.com")
        mail.login(email, "tnlr iyqz olar simf")
        print("SUCCESS! Login worked with original spaces.")
        mail.logout()
    except Exception as e:
        print(f"FAILED with spaces: {e}")

if __name__ == "__main__":
    test_app_password()
