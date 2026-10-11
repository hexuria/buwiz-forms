/* LD_PRELOAD for lavapipe in this VM: LLVM picks its JIT target from the CPU part
 * (Neoverse V2 => SVE), but the VM hides SVE, so JIT code dies with SIGILL.
 * Report a CPU without SVE instead. */
#include <stdlib.h>
#include <string.h>
struct StringRef { const char *data; size_t len; };
static const char CPU[] = "neoverse-n1";
/* llvm::sys::getHostCPUName() */
struct StringRef _ZN4llvm3sys14getHostCPUNameEv(void) { struct StringRef r = { CPU, sizeof CPU - 1 }; return r; }
/* LLVMGetHostCPUName(): caller frees with LLVMDisposeMessage (free). */
char *LLVMGetHostCPUName(void) { return strdup(CPU); }
