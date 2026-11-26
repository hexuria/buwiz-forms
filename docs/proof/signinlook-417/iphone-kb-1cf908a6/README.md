# Keyboard proof at 1cf908a6

| file | device | result |
|---|---|---|
| iphone17-email-light.png | iPhone 17, light | FAIL: no logo; “Enter your email” is cut under the back row; large empty gap between Continue and keyboard. PASS: focused email field and Continue visible. |
| iphone17-email-dark.png | iPhone 17, dark | FAIL: no logo; heading cut under the back row; large empty gap between Continue and keyboard. PASS: focused email field and Continue visible. |
| iphone17-pin-light.png | iPhone 17, light | FAIL: logo reduced to a thin clipped sliver, not whole with its words. PASS: no large gap; focused PIN box and Sign in visible. |
| iphone17-pin-dark.png | iPhone 17, dark | FAIL: logo reduced to a thin clipped sliver, not whole with its words. PASS: no large gap; focused PIN box and Sign in visible. |
| iphone-se-email-light.png | iPhone SE 375x667, light | FAIL: no logo; heading missing/cut under the back row; large empty gap between Continue and keyboard. PASS: focused email field and Continue visible. |
| iphone-se-email-dark.png | iPhone SE 375x667, dark | FAIL: no logo; heading missing/cut under the back row; large empty gap between Continue and keyboard. PASS: focused email field and Continue visible. |

All six helper test processes passed mechanically. None of the six pictures passes the visual keyboard-layout checks.
