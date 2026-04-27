import AppKit
import PDFKit

func printPDF(path: String) {
    let url = URL(fileURLWithPath: path)
    guard let pdfDoc = PDFDocument(url: url) else {
        print("Could not load PDF at \(path)")
        exit(1)
    }
    
    let printInfo = NSPrintInfo.shared
    printInfo.isHorizontallyCentered = true
    printInfo.isVerticallyCentered = true
    
    // We need to create an NSPrintOperation. For a PDFDocument, we can get a print operation:
    let printOp = pdfDoc.printOperation(for: printInfo, scalingMode: .pageScaleDownToFit, autoRotate: true)
    
    // We need a shared app to show UI
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)
    app.activate(ignoringOtherApps: true)
    
    printOp?.showsPrintPanel = true
    printOp?.showsProgressPanel = true
    
    // Run the operation
    printOp?.run()
}

let args = CommandLine.arguments
if args.count > 1 {
    printPDF(path: args[1])
}
