# Property and parent-chain dumps

## Stock EditText

```text
longClickable=true textSelectable=false focusable=true focusableInTouchMode=true movement=android.text.method.ArrowKeyMovementMethod edit.longClickListener=false touchListener=false
edit > android.widget.FrameLayout[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > android.widget.FrameLayout[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > android.widget.LinearLayout[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > com.android.internal.policy.DecorView[intercept=com.android.internal.policy.DecorView,longClickListener=false touchListener=false] > android.view.ViewRootImpl
```

## Kit InputBox EditText

```text
longClickable=true textSelectable=false focusable=true focusableInTouchMode=true movement=android.text.method.ArrowKeyMovementMethod edit.longClickListener=false touchListener=false
edit > com.longbridge.nsh.InputBox[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.ScrollBox$Content[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.ScrollBox$VScroll[intercept=com.longbridge.nsh.ScrollBox$VScroll,longClickListener=false touchListener=false] > com.longbridge.nsh.ScrollBox[intercept=com.longbridge.nsh.ScrollBox,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.NshView[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > android.widget.FrameLayout[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > android.widget.FrameLayout[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > android.widget.LinearLayout[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > com.android.internal.policy.DecorView[intercept=com.android.internal.policy.DecorView,longClickListener=false touchListener=false] > android.view.ViewRootImpl
```


# Hold interception summary

| point | run | intercept lines in the hold window | EditText ACTION_CANCEL |
|---|---:|---|---|
| p1 | 1 | Box action=0 return=false ×19; ScrollBox$VScroll action=0 return=false ×1 | no |
| p1 | 2 | Box action=0 return=false ×19; ScrollBox$VScroll action=0 return=false ×1 | no |
| p1 | 3 | Box action=0 return=false ×19; ScrollBox$VScroll action=0 return=false ×1 | no |
| p2 | 1 | Box action=0 return=false ×19; Box action=1 return=false ×12; ScrollBox$VScroll action=0 return=false ×1; ScrollBox$VScroll action=1 return=false ×1 | no |
| p2 | 2 | Box action=0 return=false ×19; Box action=1 return=false ×12; ScrollBox$VScroll action=0 return=false ×1; ScrollBox$VScroll action=1 return=false ×1 | no |
| p2 | 3 | Box action=0 return=false ×19; Box action=1 return=false ×12; ScrollBox$VScroll action=0 return=false ×1; ScrollBox$VScroll action=1 return=false ×1 | no |

No MOVE event is injected by this hold path. No logged interception returned true, and no ours run logged ACTION_CANCEL.
