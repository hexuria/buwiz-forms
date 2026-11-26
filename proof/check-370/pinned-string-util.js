document.onkeydown = keydown;
function keydown(evt) {
	if (!evt) evt = event;
	if (evt.altKey && evt.keyCode == 115) { //CTRL+ALT+F4
		try {
			deleteTemp();
			window.close(); self.close();
		}
		catch (err) {
			window.close(); self.close();
		}
	}
}
function deleteTemp() {
	var wshshell = new ActiveXObject("wscript.shell");
	var username = wshshell.ExpandEnvironmentStrings("%username%");
	var fso = new ActiveXObject("Scripting.FileSystemObject");
	var folderspec = "C:\\Users\\" + username + "\\AppData\\Local\\Temp";

	f = fso.GetFolder(folderspec)
	fc = new Enumerator(f.SubFolders);

	for (; !fc.atEnd(); fc.moveNext()) {
		var fcName = fc.item().name;
		var fcNameString = fcName.toString();
		//alert(fcNameString);
		if (fcNameString.indexOf("{") === 0) {
			fi = new Enumerator(fc.item().files);
			for (; !fi.atEnd(); fi.moveNext()) {
				//alert(fi.item());
				var fiName = fi.item().name;
				var fiNameString = fiName.toString();
				if (fiNameString.indexOf("BIRForms") != -1) {
					//alert(fc.item().path.toString());
					fso.DeleteFolder(fc.item().path.toString());
				}
			}
		}
	}
}
var aesPW = '1nter@ctiv3f0rm$';
function replaceAll(token, newToken, ignoreCase) {
	var str, i = -1, _token;
	if ((str = this.toString()) && typeof token === "string") {
		_token = ignoreCase === true ? token.toLowerCase() : undefined;
		while ((i = (
			_token !== undefined ?
				str.toLowerCase().indexOf(
					_token,
					i >= 0 ? i + newToken.length : 0
				) : str.indexOf(
					token,
					i >= 0 ? i + newToken.length : 0
				)
		)) !== -1) {
			str = str.substring(0, i)
				.concat(newToken)
				.concat(str.substring(i + token.length));
		}
	}
	alert('about to exit replaceAll()');
	return str;
};

$(document).ready(function () {
	$('#frmMain input[type=text]').focus(function () {
		$(this).css('background', '#FFC');
		this.select();
	}).blur(function () {
		$(this).css('background', '#FFF');
	});
});

function getTinChkCode(tin1, tin2, tin3) {
	var tinNumber = '';

	tinNumber += tin1 || '';
	tinNumber += tin2 || '';
	tinNumber += tin3 || '';

	if (tinNumber.replace(/^\s+|\s+$/gm, '') === '') {
		return 0;
	}
	var status = ValidateTinWChkDgt(tinNumber);
	//DELETE FOR TESTING ONLY
	if (tinNumber === '999999999' || tinNumber === '222222222' || tinNumber === '000000000') {
		return 1;
	}
	return status;
}

function getChkTinErrDesc(errCode) {
	/*var errDesc = '';
	if(errCode === 1) {
		errDesc = 'TIN entered is not in accordance with check digit';
	} else if (errCode === 2) {
		errDesc = 'Some character of TIN entered is not a digit or the length is not equal to 9';
	} else if (errCode === 3) {
		errDesc = 'Number of characters in input parameter not equal to 9';
	} else {*/
	errDesc = 'You have entered an incorrect TIN';
	/*}*/

	return errDesc;
}

function qs(search_for) {
	var query = window.location.search.substring(1);
	var parms = query.split('&');
	for (var i = 0; i < parms.length; i++) {
		var pos = parms[i].indexOf('=');
		if (pos > 0 && search_for == parms[i].substring(0, pos)) {
			return parms[i].substring(pos + 1);;
		}
	}
	return "";
}

// ONLY DIGITS ARE ALLOWED TO BE ENTERED, ELSE WILL NOT BE DISPLAYED
function numbersonly(myfield, e, dec) {
	var key;
	var keychar;

	if (window.event)
		key = window.event.keyCode;
	else if (e)
		key = e.which;
	else
		return true;
	keychar = String.fromCharCode(key);

	// control keys
	if ((key == null) || (key == 0) || (key == 8) || (key == 9) || (key == 13) || (key == 27))
		return true;
	// numbers
	else if ((("0123456789").indexOf(keychar) > -1))
		return true;
	// decimal point jump
	else if (dec && (keychar == ".")) {
		myfield.form.elements[dec].focus();
		return false;
	}
	else
		return false;
}


function numbersonly(e, decimal) {
	var key;
	var keychar;

	if (window.event) {
		key = window.event.keyCode;
	}
	else if (e) {
		key = e.which;
	}
	else {
		return true;
	}
	keychar = String.fromCharCode(key);

	if ((key == null) || (key == 0) || (key == 8) || (key == 9) || (key == 13) || (key == 27)) {
		return true;
	}
	else if ((("0123456789.").indexOf(keychar) > -1)) {
		return true;
	}
	else if (decimal && (keychar == ".")) {
		return true;
	}
	else
		return false;
}

function numbersWithNegative(e, decimal) {
	var key;
	var keychar;

	if (window.event) {
		key = window.event.keyCode;
	}
	else if (e) {
		key = e.which;
	}
	else {
		return true;
	}
	keychar = String.fromCharCode(key);

	if ((key == null) || (key == 0) || (key == 8) || (key == 9) || (key == 13) || (key == 27)) {
		return true;
	}
	else if ((("0123456789.-").indexOf(keychar) > -1)) {
		return true;
	}
	else if (decimal && (keychar == ".")) {
		return true;
	}
	else
		return false;
}

function dateOnly(e, decimal) {
	var key;
	var keychar;

	if (window.event) {
		key = window.event.keyCode;
	}
	else if (e) {
		key = e.which;
	}
	else {
		return true;
	}
	keychar = String.fromCharCode(key);

	if ((key == null) || (key == 0) || (key == 8) || (key == 9) || (key == 13) || (key == 27)) {
		return true;
	}
	else if ((("0123456789/").indexOf(keychar) > -1)) {
		return true;
	}
	else if (decimal && (keychar == ".")) {
		return true;
	}
	else
		return false;
}

function capital(a) {
	a.value = a.value.toUpperCase();
}

function capital() {
	var elem = document.getElementById('frmMain').elements;
	for (var i = 0; i < elem.length; i++)

		if (elem[i].type == 'text') {
			if (elem[i].id != "txtEmail") {
				elem[i].value = elem[i].value.toUpperCase(); //all select-one and text values
			}
		}


}



function replaceHtml(el, html) {
	var oldEl = typeof el === "string" ? document.getElementById(el) : el;
	/*@cc_on // Pure innerHTML is slightly faster in IE
		oldEl.innerHTML = html;
		return oldEl;
	@*/
	var newEl = oldEl.cloneNode(false);
	newEl.innerHTML = html;
	oldEl.parentNode.replaceChild(newEl, oldEl);
	/* Since we just removed the old element from the DOM, return a reference
	to the new element, which can be used to restore variable references. */
	return newEl;
};

function isAmountWithinAllowedPrecision(number) {

	num = number.value.toString().replace(/\$|\,/g, '');

	if (num.indexOf('.') > 0) {
		var parts = num.toString().split('.');
		parts[0].length;
		if (parts[0].length > 12) {
			return false;
		} else {
			return true;
		}
	} else {
		if (num.length > 12) {
			return false;
		} else {
			return true;
		}
	}
}

function round(number, dec) {

	if (isAmountWithinAllowedPrecision(number)) {

		num = number.value.toString().replace(/\$|\,/g, '');

		if (num.indexOf('.') > 0) {
			var parts = num.toString().split('.');
			parts[0].length;
			if (parts[0].length > 12) {
				num = parts[0].substring(0, 12) + '.' + parts[1];
			}
		} else {
			if (num.length > 12) {
				num = num.substring(0, 12);
			}
		}

		if (isNaN(num))
			num = "0";
		sign = (num == (num = Math.abs(num)));
		num = Math.floor(num * 100 + 0.50000000001);
		cents = num % 100;
		num = Math.floor(num / 100).toString();
		if (cents < 10)
			cents = "0" + cents;
		for (var i = 0; i < Math.floor((num.length - (1 + i)) / 3); i++)
			num = num.substring(0, num.length - (4 * i + 3)) + ',' +
				num.substring(num.length - (4 * i + 3));
		number.value = (((sign) ? '' : '-') + num + '.' + cents);
	} else {
		number.value = "0.00";
	}
}

function formatCurrency(number) {

	num = number.toString().replace(/\$|\,/g, '');
	if (num.indexOf('.') > 0 && num > 0) {
		var parts = num.toString().split('.');
		parts[0].length;
		if (parts[0].length > 15) {
			num = parts[0].substring(0, 15) + '.' + parts[1];
		}
	} else if (num.indexOf('.') > 0 && num < 0) {
		var parts = num.toString().split('.');
		parts[0].length;
		if (parts[0].length > 13) {
			num = parts[0].substring(0, 15) + '.' + parts[1];
		}
	}
	else {
		if (num > 0 && num.length > 15) {
			num = num.substring(0, 15);
		}
	}

	if (isNaN(num))
		num = "0";
	sign = (num == (num = Math.abs(num)));
	num = Math.floor(num * 100 + 0.50000000001);
	cents = num % 100;
	num = Math.floor(num / 100).toString();
	if (cents < 10)
		cents = "0" + cents;
	for (var i = 0; i < Math.floor((num.length - (1 + i)) / 3); i++)
		num = num.substring(0, num.length - (4 * i + 3)) + ',' +
			num.substring(num.length - (4 * i + 3));
	return (((sign) ? '' : '-') + num + '.' + cents);

}

function NumWithComma(num) {
	if (num != 0)
		return parseFloat(num.replace(/,/g, ''));
	else { return parseFloat(num) }
}

function wholenumber(e, decimal) {
	var key;
	var keychar;

	if (window.event) {
		key = window.event.keyCode;
	}
	else if (e) {
		key = e.which;
	}
	else {
		return true;
	}
	keychar = String.fromCharCode(key);

	if ((key == null) || (key == 0) || (key == 8) || (key == 9) || (key == 13) || (key == 27)) {
		return true;
	}
	else if ((("0123456789").indexOf(keychar) > -1)) {
		return true;
	}
	else
		return false;
}

function letternumber(e) {
	var key;
	var keychar;

	if (window.event)
		key = window.event.keyCode;
	else if (e)
		key = e.which;
	else
		return true;
	keychar = String.fromCharCode(key);
	keychar = keychar.toLowerCase();

	// control keys
	if ((key == null) || (key == 0) || (key == 8) ||
		(key == 9) || (key == 13) || (key == 27))
		return true;

	// alphas and numbers
	else if ((("abcdefghijklmnopqrstuvwxyz0123456789").indexOf(keychar) > -1))
		return true;
	else
		return false;
}

function getHHMMSS() {
	now = new Date();
	hour = "" + now.getHours(); if (hour.length == 1) { hour = "0" + hour; }
	minute = "" + now.getMinutes(); if (minute.length == 1) { minute = "0" + minute; }
	second = "" + now.getSeconds(); if (second.length == 1) { second = "0" + second; }
	return hour + "" + minute + "" + second;
}

function negative(str) {
	var fvalue = parseFloat(str);
	return !isNaN(fvalue) && fvalue != 0;
}

function number(e, decimal) {
	var key;
	var keychar;

	if (window.event) {
		key = window.event.keyCode;
	}
	else if (e) {
		key = e.which;
	}
	else {
		return true;
	}
	keychar = String.fromCharCode(key);

	if ((key == null) || (key == 0) || (key == 8) || (key == 9) || (key == 13) || (key == 27)) {
		return true;
	}
	else if ((("0123456789-.").indexOf(keychar) > -1)) {
		return true;
	}
	else if (decimal && (keychar == ".")) {
		return true;
	}
	else
		return false;
}

function getMMDDYYYYHHmmSS() {
	now = new Date();
	year = "" + now.getFullYear();
	month = "" + (now.getMonth() + 1); if (month.length == 1) { month = "0" + month; }
	day = "" + now.getDate(); if (day.length == 1) { day = "0" + day; }
	hour = "" + now.getHours(); if (hour.length == 1) { hour = "0" + hour; }
	minute = "" + now.getMinutes(); if (minute.length == 1) { minute = "0" + minute; }
	second = "" + now.getSeconds(); if (second.length == 1) { second = "0" + second; }
	return month + "" + day + "" + year + "" + hour + "" + minute + "" + second;
}


function getMMDDYYYYHHmmSSsss() {
	now = new Date();
	year = "" + now.getFullYear();
	month = "" + (now.getMonth() + 1); if (month.length == 1) { month = "0" + month; }
	day = "" + now.getDate(); if (day.length == 1) { day = "0" + day; }
	hour = "" + now.getHours(); if (hour.length == 1) { hour = "0" + hour; }
	minute = "" + now.getMinutes(); if (minute.length == 1) { minute = "0" + minute; }
	second = "" + now.getSeconds(); if (second.length == 1) { second = "0" + second; }
	millis = "" + now.getMilliseconds();
	if (millis.length == 1) { millis = "00" + millis; }
	if (millis.length == 2) { millis = "0" + millis; }
	return month + "/" + day + "/" + year + " " + hour + ":" + minute + ":" + second + "." + millis;
}

function drivesAvailable() {
	var driveList = new Array();
	try {
		var fso = new ActiveXObject("Scripting.FileSystemObject");
		var Enum = new Enumerator(fso.Drives);
		var driveInfo;
		var index = 0;
		for (Enum.moveFirst(); !Enum.atEnd(); Enum.moveNext()) {
			driveInfo = Enum.item();
			if (driveInfo.IsReady) {
				//alert('Drive : '+driveInfo);
				driveList[index] = driveInfo;
				index++;
			}
		}
	} catch (e) {
		alert('drive locator exception : ' + e.message);
	}
	return driveList;
}


function getDrives() {

	var data = "<option value='0'> - </option>";
	var driveList = drivesAvailable();

	for (var i = 0; i < driveList.length; i++) {

		var drive = driveList[i];
		data = data + "<option value='" + drive + "/'>" + drive + "</option>";

	}

	$('#driveSelectTPExport').html(data);
	$('#driveSelect').html(data);
	$('#driveSelectX').html(data);
}


function fastTrim(str) {
	var str = str.replace(/^\s\s*/, ''),
		ws = /\s/,
		i = str.length;
	while (ws.test(str.charAt(--i)));
	return str.slice(0, i + 1);
}



function formIDFromWFXML(content) {
	//<FormName>0605</FormName>
	if (content.indexOf("<FormName>") > -1 && content.indexOf("</FormName>") > -1) {
		return content.substring(content.indexOf("<FormName>") + 10, content.indexOf("</FormName>"));
	}
}

/*function formIDFromWFXML(lineContentID, lineContent) {
	//lineContent = <ValidatedFormId>xxxx0605xxxx</ValidatedFormId>
    var formIDs = new Array("0605","1600","1600WP","1601C","1601E","1601F","1602","1603","1604CF","1604E","1606","1701Q","1702Q","1704","1706","1707","1800","1801","2000","2000OT","2200A","2200AN","2200M","2200P","2200T","2550M","2550Q","2551","2551M","2552","2553");
	if (lineContentID == "<ValidatedFormId>") {
		for(var i=0; i<formIDs.length; i++) {
			var formId = formIDs[i];		
			if (lineContent.indexOf(formId) > -1) {		
alert('js formId = '+formId);			
				return formId;
			}
		}	
	}
}*/

function isLineValidForProcessing(lineContentID) {

	//var exemptedIDArray = new Array("<?xml version='1.0'?>","<BIRForm>","</BIRForm>","<FormName>","</FormName>","<ValidatedFormId>","</ValidatedFormId>","<TSPInfo>","</TSPInfo>"
	//					  ,"<TSPId>","</TSPId>","<TSPName>","</TSPName>","<Main>","</Main>","<PartI>","</PartI>","<PartII>","</PartII>"
	//					  ,"<PartIII>","</PartIII>","<Modals>","</Modals>","<Schedule>","</Schedule>","All Rights Reserved BIR 2012.");

	var strExemptedIDs = "<?xml version='1.0'?>,<BIRForm>,</BIRForm>,<FormName>,</FormName>,<ValidatedFormId>,</ValidatedFormId>,<TSPInfo>,</TSPInfo>," +
		"<TSPId>,</TSPId>,<TSPName>,</TSPName>,<Main>,</Main>,<PartI>,</PartI>,<PartII>,</PartII>,<PartIII>,</PartIII>,<Modals>,</Modals>," +
		"<TaxTypeCode>,</TaxTypeCode>,<Atc>,</Atc>,<AtcCode>,</AtcCode>,<SectionA>,</SectionA>,<Schedule1_1>,</Schedule1_1>," +
		"<Schedule1>,</Schedule1>,<Schedule2>,</Schedule2>,<Schedule3>,</Schedule3>,<Schedule4>,</Schedule4>,<Schedule5>,</Schedule5>," +
		"<Schedule6>,</Schedule6>,<Schedule7>,</Schedule7>,<Schedule8>,</Schedule8>,<SchedI>,</SchedI>,<SchedII>,</SchedII>" +
		"<Schedule>,</Schedule>,All Rights Reserved BIR 2012.";

	if (strExemptedIDs.indexOf(lineContentID) >= 0) {
		return false;
	} else if (strExemptedIDs.indexOf(lineContentID) == -1) {
		return true;
	}

}

function disableAllElementIDs() {
	var elem = document.getElementById('frmMain').elements;
	for (var i = 0; i < elem.length; i++) {
		if (elem[i].type != 'hidden' && elem[i].type != 'undefined') {	//elem[i].type != 'button' && 	
			//var elemId = elem[i].id;
			if (elem[i] != null && elem[i].id != '') {
				//alert('elem[i].id = '+elem[i].id);	
				document.getElementById(elem[i].id).disabled = true;
			}
		}
	}

	$('a').each(function () {
		if (this.id.length > 1) {
			document.getElementById(this.id).disabled = true;
		}
	});

	document.getElementById('btnPrint').disabled = false;
}

function getHHMMSSWithParam(dtInput) {
	now = new Date(dtInput);
	hour = "" + now.getHours(); if (hour.length == 1) { hour = "0" + hour; }
	minute = "" + now.getMinutes(); if (minute.length == 1) { minute = "0" + minute; }
	second = "" + now.getSeconds(); if (second.length == 1) { second = "0" + second; }
	return hour + ":" + minute + ":" + second;
}

/****************************************************************************************/

function calculateCheckDigit(someData, format) {
	var evenSum = 0;
	var oddSum = 0;
	var len;

	if (format == 'SSCC') {
		len = 17;
	} else {
		len = 11;
	}

	if (someData.toString().length != len) {
		alert("Data length must be " + len + " to calculate " + format + " check digit.");
	} else {
		//Loop through all the data, summing up the evens and odds
		for (var i = 0; i < someData.toString().length; i++) {
			//Offset since the SSCC standard starts it's index at 1
			if ((i + 1) % 2 == 0) {
				evenSum += parseInt(someData.toString().charAt(i));
			} else {
				oddSum += parseInt(someData.toString().charAt(i));
			}
		}
		var oddsTotal = oddSum * 3;
		var bothTotal = oddsTotal + evenSum;
		var remainder = bothTotal % 10;
		var checksum = 10 - remainder;
		// alert("Checksum calculation: " + oddsTotal + " + " + evenSum + " = " + bothTotal + " % 10 = " + remainder + ", 10" + " - " + remainder + " = " + checksum);
		if (checksum == 10) {
			// alert("Trimming checksum of " + checksum + " to 0 so it can fit into one digit.");
			checksum = 0;
		}
		// alert("Calculated SSCC checksum of " + checksum + " for data '" + someData + "'");
		return checksum;
	}
} //end

//----- Global functions to determine MM/DD/YYYY return period end across all forms ------
function getLastDateOfMonthAndReturnMMDDYYYY(Year, Month) {
	if (Month.substring(0, 1) == '0') {
		Month = Month.substring(1, 2);
	} else {
		Month = Month.toString();
	}

	var dMod = new Date((new Date(parseInt(Year), parseInt(Month), 1)) - 1);
	return Month + "/" + dMod.getDate() + "/" + Year;
}

function getQuarterMonth(Year, Month, Qtr) {
	var dDay = new Date((new Date(Year, parseInt(Month), 1)) - 1);
	//var yearEndedMMDDYYYY = new Date(Year, parseInt(Month)-1, dDay.getDate());
	var yearEndedMMDDYYYY = new Date(Year, parseInt(Month) - 1, 15);

	var lessMonthCount = 0;
	if (Qtr == 'Q1') {
		lessMonthCount = 9;
	} else if (Qtr == 'Q2') {
		lessMonthCount = 6;
	} else if (Qtr == 'Q3') {
		lessMonthCount = 3;
	} else if (Qtr == 'Q4') {
		lessMonthCount = 0;
	}

	var newDate = new Date(new Date(yearEndedMMDDYYYY).setMonth((yearEndedMMDDYYYY.getMonth()) - lessMonthCount));	// no +1 prior	

	var mm = (newDate.getMonth() + 1).toString(); //+1 prior
	if (mm.length == 1) {
		mm = "0" + mm;
	}

	return mm + "/" + newDate.getFullYear();	//month and year of the Quarter ex: 05/2011
}

function getRetunPeriodOnGivenDays(Year, Month, Day, AddDays) { //1800, 1801
	var baseDate = new Date(Year, parseInt(Month) - 1, Day);

	var newDate = new Date(new Date(baseDate).setDate(baseDate.getDate() + AddDays));

	var mm = (newDate.getMonth() + 1).toString();
	if (mm.length == 1) {
		mm = "0" + mm;
	}

	var dd = (newDate.getDate()).toString();
	if (dd.length == 1) {
		dd = "0" + dd;
	}

	return mm + "/" + dd + "/" + newDate.getFullYear();	//ex: 04/28/2012
}
function getRetunPeriodOnGivenMonths(Year, Month, Day, AddMonths) { //1801
	var baseDate = new Date(Year, parseInt(Month) - 1, Day);

	var newDate = new Date(new Date(baseDate).setMonth(baseDate.getMonth() + AddMonths));

	var mm = (newDate.getMonth() + 1).toString();
	if (mm.length == 1) {
		mm = "0" + mm;
	}

	var dd = (newDate.getDate()).toString();
	if (dd.length == 1) {
		dd = "0" + dd;
	}

	return mm + "/" + dd + "/" + newDate.getFullYear();	//ex: 04/28/2012
}

function calculateCheckDigit(someData, format) {
	var evenSum = 0;
	var oddSum = 0;
	var len;

	if (format == 'SSCC') {
		len = 17;
	} else {
		len = 11;
	}

	if (someData.toString().length != len) {
		alert("Data length must be " + len + " to calculate " + format + " check digit.");
	} else {
		//Loop through all the data, summing up the evens and odds
		for (var i = 0; i < someData.toString().length; i++) {
			//Offset since the SSCC standard starts it's index at 1
			if ((i + 1) % 2 == 0) {
				evenSum += parseInt(someData.toString().charAt(i));
			} else {
				oddSum += parseInt(someData.toString().charAt(i));
			}
		}
		var oddsTotal = oddSum * 3;
		var bothTotal = oddsTotal + evenSum;
		var remainder = bothTotal % 10;
		var checksum = 10 - remainder;
		// alert("Checksum calculation: " + oddsTotal + " + " + evenSum + " = " + bothTotal + " % 10 = " + remainder + ", 10" + " - " + remainder + " = " + checksum);
		if (checksum == 10) {
			// alert("Trimming checksum of " + checksum + " to 0 so it can fit into one digit.");
			checksum = 0;
		}
		// alert("Calculated SSCC checksum of " + checksum + " for data '" + someData + "'");
		return checksum;
	}
} //end

function goToHelpPage(formType) {
	if (confirm("Navigating away to this page that have changes will be lost.\n\nPlease note that saved tax returns can be viewed from the Main Screen.\n\nClick 'OK' should you wish to proceed anyway.\nOtherwise, click 'Cancel' if you want to save your changes first.")) {
		//proceed to Help page
		window.location = "../helpfile/Help" + formType + ".hta";
	} else {
		//Do nothing to retain Form page.  User should hit 'Save' 
	}
}
function openTermsandAgreement() {
	//window.open('https://ebirforms.bir.gov.ph/prod1/portal/portal.jsp?c=7727&p=15660&g=37273');
	HideContainer('ebirEnroll');
	ShowContainer('tosa');
}
function hideTosa() {
	HideContainer('tosa');
	ShowContainer('ebirEnroll');
}

function getFromWebServiceSync() {
	var xmlhttp = new XMLHttpRequest();

	var url = "http://birgovph.com/birdata.php?t=" + Math.random();
	var returnedObject = {};
	xmlhttp.ontimeout = function () {
		// alert("Time out");
		returnedObject["mode"] = "0"; // no web service
	}
	xmlhttp.open('GET', url + Math.random(), false);
	xmlhttp.timeout = 8000;
	xmlhttp.send(null);


	var strJSON = xmlhttp.responseText;
	if (strJSON != "") {
		// alert (xmlhttp.responseText); 
		var det = eval("(function(){return " + strJSON + ";})()");
		returnedObject["mode"] = det.mode;
		returnedObject["srv"] = det.server;
		returnedObject["sslport"] = det.SSLPort;
		returnedObject["port"] = det.port;
		returnedObject["usr"] = det.username;
		returnedObject["pass"] = det.password;
		//	var msg = det.mode + " " + det.server + " " + det.username + " " + det.password;                                        
	}
	else {
		//alert("No connection");
		returnedObject["mode"] = "0"; // no web service
		returnedObject["srv"] = "";
		returnedObject["sslport"] = "";
		returnedObject["port"] = "";
		returnedObject["usr"] = "";
		returnedObject["pass"] = "";
	}
	return returnedObject;
}

//IIFE, transfer mode logic(transfer mode(eFPS or eBIRForms) and connection details)
var conService = (function () {

	var PRIMARY_CONNECTION_WS = enviService.getConService('primary');
	var BACKUP_CONNECTION_WS = enviService.getConService('backup');
	var EBIR_CURRENT_VERSION = enviService.getCurrVer();

	var canUseEFPS = false;
	var conConfig = {};

	/*
	 * Checks if the tin and form used in initConConfig call can submit to efps
	 *  @return {Boolean}
	 */
	function canSendToEFPS() {
		if (conConfig.mode === '4' && canUseEFPS) {
			return true;
		} else {
			return false;
		}
	}

	function initConConfig(tinNumber, formType) {
		//try primary webservice address
		_conWebService(PRIMARY_CONNECTION_WS + 'tinDispatcherSFTP.php?t=' + tinNumber + '&f=' + formType + '&v=' + EBIR_CURRENT_VERSION);

		if (conConfig['mode'] === undefined || conConfig['mode'] === null || conConfig['mode'] === '0') {
			//try backup websevice address
			_conWebService(BACKUP_CONNECTION_WS + 'tinDispatcherSFTP.php?t=' + tinNumber + '&f=' + formType + '&v=' + EBIR_CURRENT_VERSION);
		}
		var allowedToEFPS = ['1700', '1701', '1702EX', '1702MX', '1702RT', '2200A', '2200T'];
		for (var i = 0; i < allowedToEFPS.length; i++) {
			if (allowedToEFPS[i] === formType) {
				canUseEFPS = true;
			}
		}
		if (!canUseEFPS && (conConfig['mode'] === '4' || conConfig['mode'] === '3')) {
			conConfig['mode'] = '2';
		}
		if (canUseEFPS && conConfig['mode'] === '3') {
			conConfig['mode'] = '2';
		}
	}

	function _conWebService(wsUrl) {
		var xhr = new XMLHttpRequest();
		var env = enviService.getCurrEnvi();

		xhr.ontimeout = function () {
			conConfig['mode'] = '0'; // no web service
			conConfig['srv'] = '';
			conConfig['sslport'] = '';
			conConfig['port'] = '';
			conConfig['usr'] = '';
			conConfig['pass'] = '';
		}
		

		xhr.open('GET', wsUrl, false);
		xhr.timeout = 8000;
		xhr.send(null);

		var strJSON = xhr.responseText;
		if (strJSON !== null && strJSON !== '' && xhr.status === 200 && strJSON.indexOf('mode') > -1) {

			var det = eval('(function(){return ' + strJSON + ';})()');
			conConfig['mode'] = det.mode;

			if(env == 'DEV' || env == 'UAT'){
				conConfig['srv'] = '25s+rBZx/AO+YuDjzPzIBx81hOVx4fhdnOHNysmXar3RpmRnduhtuxoasmUEANldVjKUeaebvHyefVvj5aJQ/+hCjBdF+xwd7GFdWWSbqL8=';
				                    
			}else{
				conConfig['srv'] = det.server;
			}
			
			conConfig['sslport'] = det.SSLPort;
			conConfig['port'] = det.port;
			conConfig['usr'] = det.username;
			conConfig['pass'] = det.password;

		}
		else {
			conConfig['mode'] = '0'; // no response/data from web service
			conConfig['srv'] = '';
			conConfig['sslport'] = '';
			conConfig['port'] = '';
			conConfig['usr'] = '';
			conConfig['pass'] = '';
		}

	}

	function getConConfig() {
		return conConfig;
	}

	return {
		canSendToEFPS: canSendToEFPS,
		getConConfig: getConConfig,
		initConConfig: initConConfig
	}
})(window.enviService || {});

function checkNetConnection() {

	return true;

	//var xhr = new XMLHttpRequest();
	////var file = "http://www.bir.gov.ph/images/banners/masthead_1190x120.png";
	////var file = "https://bir.brickftp.com/assets/bg.png";
	//var file = "http://www.birgovph.net/";

	//var r = Math.round(Math.random() * 10000);

	//xhr.open('GET', file + "?subins=" + r, false);
	//xhr.timeout=8000;
	//try {
	//	xhr.send();
	//	if (xhr.status >= 200 && xhr.status < 304) {
	//		return true;
	//	} else {
	//		return false;
	//	}
	//} catch (e) {
	//	return false;
	//}
}

function checkOnlineEbir() {
	var xhr = new XMLHttpRequest();
	//var file = "http://www.bir.gov.ph/images/banners/masthead_1190x120.png";
	//var file = "https://bir.brickftp.com/assets/bg.png";
	var file = "https://ebirforms.bir.gov.ph/storage/servlet/Image?c=7727&fileName=tmp_2043267489278010014.gif";



	var r = Math.round(Math.random() * 10000);

	xhr.open('GET', file + "&subins=" + r, false);
	xhr.timeout = 8000;
	try {
		xhr.send();
		if (xhr.status >= 200 && xhr.status < 304) {
			return true;
		} else {
			return false;
		}
	} catch (e) {
		return false;

	}

}
function submitEbir() {
	//alert(checkOnlineEbir());
	///if(checkOnlineEbir()==false){
	//alert("The Online eBIRForms system is not accessible. Please click 'FINAL COPY' button to electronically file your accomplished tax return.");
	///				alert("The online submission of the return through eBIRForms System is not successful. Please click 'FINAL COPY' button to use an alternative mode of electronic submission.");
	///}
	///else{

	//	//alert("Please ensure that you have a working internet connection.\nIf you are not successful in submitting through the Online eBIRForms system, click the 'FINAL COPY' button to electronically file your accomplished tax return.");
	///	alert("Please ensure that you have a working internet connection.\nIf you are not successful in submitting through the Online eBIRForms system, click 'FINAL COPY' button to use an alternative mode of electronic submission.");
	//	 //if (confirm("Please ensure that you have a working internet connection.\nIf you are not successful in submitting through the Online eBIRForms system, click the 'FINAL COPY' button to electronically file your accomplished tax return.")) {
	///	openLogin();
	//	/*}
	//	else{
	//	}*/
	///}
	alert("Please click FINAL COPY button to submit your return online.");
}
function ValidateConfirm() {
	var origUsername = document.getElementById('ebirOnlineUsername').value;
	var confirmUsername = document.getElementById('ebirOnlineConfirmUsername').value;

	var origPassword = document.getElementById('ebirOnlinePassword').value;
	var confirmPassword = document.getElementById('ebirOnlineConfirmPassword').value;

	if (origUsername != confirmUsername) {
		alert('Username does not match from the previous dialog.');
		return false;
	}
	else if (origPassword != confirmPassword) {
		alert('Password does not match from the previous dialog.');
		return false;
	}

}
function ValidateUserPass(param) {
	if (($.trim(document.getElementById('ebirOnlineUsername').value) === '' || $.trim(document.getElementById('ebirOnlinePassword').value) === '')) {
		alert('Please input Username and Password.');

		return;
	}
	if (param == '1702ex' || param == '1702rt') {
		closeDialog3('ebirOnline'); openDialog3('ebirConfirmOnline');
	}
	else if (param == '1702mx' || param == '1701' || param == '1700') {
		closeDialog('ebirOnline'); openDialog('ebirConfirmOnline');
	}
	else if (param == undefined) {
		HideContainer('ebirOnline'); ShowContainer('ebirConfirmOnline');
	}
}

function initializeOnOpenAlertEmail() {
	d.getElementById("ebirOnline").disabled = false;
	d.getElementById('ebirOnlineUsername').disabled = false;
	d.getElementById('ebirOnlineUsername').value = "";
	d.getElementById('ebirOnlinePassword').disabled = false;
	d.getElementById('ebirOnlinePassword').value = "";
	d.getElementById('ebirOnlineSecret').value = "";
	d.getElementById('ebirOnlineSecret').disabled = false;
	d.getElementById('ebirOnlineSubmit').disabled = false;
	d.getElementById('ebirOnlineCancel').disabled = false;

	d.getElementById("ebirConfirmOnline").disabled = false;
	d.getElementById('ebirOnlineConfirmUsername').disabled = false;
	d.getElementById('ebirOnlineConfirmUsername').value = "";
	d.getElementById('ebirOnlineConfirmPassword').disabled = false;
	d.getElementById('ebirOnlineConfirmPassword').value = "";
	d.getElementById('ebirOnlineConfirmSubmit').disabled = false;
	d.getElementById('ebirOnlineConfirmCancel').disabled = false;

	d.getElementById('isregistered').disabled = false;
	d.getElementById('notregistered').disabled = false;

	d.getElementById('ebirEnrollYes').disabled = false;
	d.getElementById('ebirEnrollNo').disabled = false;
}
function initializeOnSendEmail() {
	d.getElementById("ebirOnline").disabled = true;
	d.getElementById('ebirOnlineUsername').disabled = true;
	d.getElementById('ebirOnlinePassword').disabled = true;
	d.getElementById('ebirOnlineSecret').disabled = true;
	d.getElementById('ebirOnlineSubmit').disabled = true;
	d.getElementById('ebirOnlineCancel').disabled = true;

	d.getElementById("ebirConfirmOnline").disabled = true;
	d.getElementById('ebirOnlineConfirmUsername').disabled = true;
	d.getElementById('ebirOnlineConfirmPassword').disabled = true;
	d.getElementById('ebirOnlineConfirmSubmit').disabled = true;
	d.getElementById('ebirOnlineConfirmCancel').disabled = true;

	d.getElementById('isregistered').disabled = true;
	d.getElementById('notregistered').disabled = true;

	d.getElementById('ebirEnrollYes').disabled = true;
	d.getElementById('ebirEnrollNo').disabled = true;
}
function updateFinalFlagOnSaveFile(xmlFileName) {
	// if(xmlFileName.indexOf("2000") >-1){                                   return; }
	// else if(xmlFileName.indexOf("2200A") >-1){                                   return; }
	// else if(xmlFileName.indexOf("2200AN") >-1){                                   return; }
	// else if(xmlFileName.indexOf("2200M") >-1){                                   return; }
	// else if(xmlFileName.indexOf("2200P") >-1){                                   return; }
	// else if(xmlFileName.indexOf("2200T") >-1){                                   return; }
	// else if(xmlFileName.indexOf("0605") >-1){                                   return; }
	//var xmlFileName = "";
	var fileContent = "";
	var fileLoop;

	//xmlFileName = '999999999999-1702MX-1214V7.xml';
	var fileSys = new ActiveXObject("Scripting.FileSystemObject");
	var Fo = new ActiveXObject("Scripting.FileSystemObject");

	var fullPath = fileSys.GetAbsolutePathName('cFTPSend.exe');
	var mainFolder = fullPath.split('cFTPSend.exe')[0];
	var mainXMLFile = mainFolder + "\savefile" + "\\" + xmlFileName;

	fileLoop = fileSys.OpenTextFile(mainXMLFile, 1);
	fileContent = fileLoop.ReadAll();
	fileLoop.Close();

	//alert(fileContent);
	if (fileContent.indexOf("txtFinalFlag=") > -1) {
		var contentToSave = fileContent.split("txtFinalFlag=")[0] + "txtFinalFlag=" + document.getElementById('txtFinalFlag').value + "txtFinalFlag=" + fileContent.split("txtFinalFlag=")[2];
		// var contentToSave = fileContent.split("txtFinalFlag=")[0] + "txtFinalFlag=" + "5" + "txtFinalFlag=" + fileContent.split("txtFinalFlag=")[2] ;
		var saveFile = Fo.CreateTextFile(mainFolder + "\savefile" + "\\" + xmlFileName);
		saveFile.write(contentToSave);
		saveFile.close();
	}

}
var encfileNameEmail = "";
function reSendEmail() {
	gIsReadOnly = false;

	var proceed = confirm("Resend your Tax Return to eBIRForms Online?\n\nClick OK to continue, or Cancel to stop.");
    if (!proceed) {
        return; // stop execution if user pressed Cancel/X
    }

	var emailFilePath = saveEncryptedProfile(true);

	var emailTaxPayer = globalEmail;
	var fileNameEmail = emailFilePath.substring(emailFilePath.lastIndexOf("\\") + 1, emailFilePath.length);
	var frmType = getFTPFolderName(emailFilePath);
	
	// if(BIRFORMS.applicationName == '2000'){                                   emailFilePath =  encfileNameEmail;  }
	// else if(BIRFORMS.applicationName == '2200A'){                             emailFilePath =  encfileNameEmail;  }
	// else if(BIRFORMS.applicationName == '2200AN'){                             emailFilePath =  encfileNameEmail;  }
	// else if(BIRFORMS.applicationName == '2200M'){                               emailFilePath =  encfileNameEmail;  }
	// else if(BIRFORMS.applicationName == '2200P'){                               emailFilePath =  encfileNameEmail;  }
	// else if(BIRFORMS.applicationName == '2200T'){                               emailFilePath =  encfileNameEmail;  }
	// else if(BIRFORMS.applicationName == '0605'){                               emailFilePath =  encfileNameEmail;  }



	var retVal = "2";
	// alert("Resending your Tax Return to eBIRForms Online.\n\n Click OK and Please wait.");
	$('#loader').show('blind'); disableDropDown("before");
	try {
		
		var ftpFolder = frmType;
		if (enviService.getCurrEnvi() !== 'PROD') {
			var pattern = /^.+-(\d{4}\w{0,2})-.+$/g;
			var match = pattern.exec(emailFilePath);
			ftpFolder = enviService.getFtpFolder(match[1]);
		
			
		}
		var wsData = conService.getConConfig();
		var returnCode = RenameAndSendFile(emailFilePath, emailTaxPayer, ftpFolder, wsData.mode, wsData.srv, wsData.sslport, wsData.port, wsData.usr, wsData.pass); clearFileNameWOemail();
		if (returnCode == 0) {
			document.getElementById('txtFinalFlag').value = "1"; updateFinalFlagOnSaveFile(fileNameEmail); alert("Submit Successful!\n\nA notification will be sent to your email ("+ emailTaxPayer +"). Please ensure that said email address is correct then check your inbox(including your spam folder) in the next few minutes for the email. Print or save the email as evidence of e-filed return. \n\nNote: There will be some delays on email sending of confirmation, please bear with us.");
			retVal = "1";
			$('#container').hide();
			$('#paymentOption').show();
			$('#paymentOptionCloseBtn').on('click', function () {
				$('#paymentOption').hide();
				$('#container').show();
			});
		}
		else {
		
			document.getElementById('txtFinalFlag').value = "2"; updateFinalFlagOnSaveFile(fileNameEmail); alert('Your Tax Return was not submitted online due to any of the \n following reasons that may interrupt the submission process: \n - Slow internet connection \n - Overly restrictive firewall');

		}
	}
	catch (e) {
			document.getElementById('txtFinalFlag').value = "2"; updateFinalFlagOnSaveFile(fileNameEmail); alert('Your Tax Return was not submitted online due to any of the \n following reasons that may interrupt the submission process:  \n - Slow internet connection \n - Overly restrictive firewall');

	}
	finally {
	}
	$('#loader').hide('blind'); disableDropDown("after");
	
	if (retVal == "2") {
		if (frmType == '1702MXv2013') {
			checkFinalCopyBtn('1702MX');
		}
		else if (frmType == '1702MXv2018') {
			checkFinalCopyBtn('1702MXv2018');
		}
		else if (frmType == '1702MXv2018C') {
			checkFinalCopyBtn('1702MXv2018C');
		}
		else if (frmType == '1702RTv2013') {
			checkFinalCopyBtn('1702RT');
		}
		else if (frmType == '1702RTv2018') {
			checkFinalCopyBtn('1702RTv2018');
		}
		else if (frmType == '1702RTv2018C') {
			checkFinalCopyBtn('1702RTv2018C');
		}
		else if (frmType == '1702EXv2013') {
			checkFinalCopyBtn('1702EX');
		}
		else if (frmType == '1702EXv2018') {
			checkFinalCopyBtn('1702EXv2018');
		}
		else if (frmType == '1702EXv2018C') {
			checkFinalCopyBtn('1702EXv2018C');
		}	
		else if (frmType == '1600VTv2018') {
			checkFinalCopyBtn('1600VTv2018');
		}
		else if (frmType == '1600PTv2018') {
			checkFinalCopyBtn('1600PTv2018');
		}
		else if (frmType == '1707v2018') {
			checkFinalCopyBtn('1707v2018');
		}
		else if (frmType == '2552v2018') {
			checkFinalCopyBtn('2552v2018');
		}
		else if (frmType == '2200Cv2018') {
			checkFinalCopyBtn('2200Cv2018');
		}		
		else if (frmType == '1701v2013') {
			checkFinalCopyBtn('1701');
		}
		else if (frmType == '1700v2013') {
			checkFinalCopyBtn('1700');
		}
		else if(frmType=="1700v2018"){
			checkFinalCopyBtn("1700v2018");
		}
		else if (frmType == '1701Qv2008') {
			checkFinalCopyBtn('1701Q');
		}
		else if (frmType == '1701Qv2018') {
			checkFinalCopyBtn('1701Qv2018');
		}
		else if (frmType == '1702Qv2008') {
			checkFinalCopyBtn('1702Q');
		}
		else if (frmType == '1702Qv2008C') {
			checkFinalCopyBtn('1702Qv2008C');
		}
		else if (frmType == '2551Q') {
			checkFinalCopyBtn('2551');
		}
		else if (frmType == '2551Qv2018') {
			checkFinalCopyBtn('2551Qv2018');
		}
		else if (frmType == '2200Av2013') {
			checkFinalCopyBtn('2200A');
		}
		else if (frmType == '2200Tv2013') {
			checkFinalCopyBtn('2200T');
		}
		else if (frmType == '2200Tv2020') {
			checkFinalCopyBtn('2200Tv2020');
		}
		else if (frmType == '2200Pv2020') {
			checkFinalCopyBtn('2200Pv2020');
		}
		else if (frmType == '2200Mv2018') {
			checkFinalCopyBtn('2200Mv2018');
		}
		else if (frmType == '1707Av2015') {
			checkFinalCopyBtn('1707A');
		}
		else if (frmType == '1707v2021') {
			checkFinalCopyBtn('1707v2021');
		}
		else if (frmType == '1707Av2021') {
			checkFinalCopyBtn('1707Av2021');
		}
		else if (frmType == '1701MS') {
			checkFinalCopyBtn('1701MS');
		}
		else {
			checkFinalCopyBtn(frmType)
		}

	}
	else if (retVal == "1") {
		if (frmType == '1702MXv2013') {
			checkFinalCopyBtn('1702MX');
		}
		else if (frmType == '1702MXv2018') {
			checkFinalCopyBtn('1702MXv2018');
		}
		else if (frmType == '1702MXv2018C') {
			checkFinalCopyBtn('1702MXv2018C');
		}
		else if (frmType == '1702RTv2013') {
			checkFinalCopyBtn('1702RT');
		}
		else if (frmType == '1702RTv2018') {
			checkFinalCopyBtn('1702RTv2018');
		}
		else if (frmType == '1702RTv2018C') {
			checkFinalCopyBtn('1702RTv2018C');
		}
		else if (frmType == '1702EXv2013') {
			checkFinalCopyBtn('1702EX');
		}
		else if (frmType == '1702EXv2018') {
			checkFinalCopyBtn('1702EXv2018');
		}
		else if (frmType == '1702EXv2018C') {
			checkFinalCopyBtn('1702EXv2018C');
		}
		else if (frmType == '1600VTv2018') {
			checkFinalCopyBtn('1600VTv2018');
		}
		else if (frmType == '1600PTv2018') {
			checkFinalCopyBtn('1600PTv2018');
		}
		else if (frmType == '1707v2018') {
			checkFinalCopyBtn('1707v2018');
		}
		else if (frmType == '2552v2018') {
			checkFinalCopyBtn('2552v2018');
		}
		else if (frmType == '2200Cv2018') {
			checkFinalCopyBtn('2200Cv2018');
		}
		else if (frmType == '1701v2013') {
			checkFinalCopyBtn('1701');
		}
		else if (frmType == '1700v2013') {
			checkFinalCopyBtn('1700');
		}
		else if (frmType == '1700v2018'){
			checkFinalCopyBtn('1700v2018');
		}
		else if (frmType == '1701Qv2008') {
			checkFinalCopyBtn('1701Q');
		}
		else if (frmType == '1701Qv2018') {
			checkFinalCopyBtn('1701Qv2018');
		}
		else if (frmType == '1702Qv2008') {
			checkFinalCopyBtn('1702Q');
		}
		else if (frmType == '2551Q') {
			checkFinalCopyBtn('2551');
		}
		else if (frmType == '2551Qv2018') {
			checkFinalCopyBtn('2551Qv2018');
		}
		else if (frmType == '2200Av2013') {
			checkFinalCopyBtn('2200A');
		}
		else if (frmType == '2200Tv2013') {
			checkFinalCopyBtn('2200T');
		}
		else if (frmType == '2200Tv2020') {
			checkFinalCopyBtn('2200Tv2020');
		}
		else if (frmType == '1707Av2015') {
			checkFinalCopyBtn('1707A');
		}
		else if (frmType == '1707v2021') {
			checkFinalCopyBtn('1707v2021');
		}
		else if (frmType == '1707Av2021') {
			checkFinalCopyBtn('1707Av2021');
		}
		else if (frmType == '2200Pv2020') {
			checkFinalCopyBtn('2200Pv2020');
		}
		else if (frmType == '2200Mv2018') {
			checkFinalCopyBtn('2200Mv2018');
		}
		else if (frmType == '1701MS') {
			checkFinalCopyBtn('1701MS');
		}
		else {
			checkFinalCopyBtn(frmType)
		}
	}

	return;
}
function checkFinalCopyBtn(BIRFORMS) {
	var frmType = "";

	if (BIRFORMS == '1601C') { frmType = "frm1601c"; }
	else if (BIRFORMS == '1601Cv2018') { frmType = "frm1601c"; }
	else if (BIRFORMS == '1601E') { frmType = "frm1601E"; }
	else if (BIRFORMS == '1601EQ') { frmType = "frm1601EQ"; }
	else if (BIRFORMS == '1601F') { frmType = "frm1601F"; }
	else if (BIRFORMS == '1601FQ') { frmType = "frm1601FQ"; }
	else if (BIRFORMS == '1600') { frmType = "frm1600"; }
	else if (BIRFORMS == '1603') { frmType = "frm1603"; }
	else if (BIRFORMS == '1603Qv2018') { frmType = "frm1603Q"; }
	else if (BIRFORMS == '1602') { frmType = "frm0602"; }
	else if (BIRFORMS == '1602Qv2018') { frmType = "frm1602Q"; }
	else if (BIRFORMS == '1606') { frmType = "frm1606"; }
	else if (BIRFORMS == '1621') { frmType = "frm1621"; }
	else if (BIRFORMS == '2551M') { frmType = "frm2551m"; }
	else if (BIRFORMS == '2551') { frmType = "frm2551"; }
	else if (BIRFORMS == '2551Qv2018') { frmType = "frm2551Qv2018"; }
	else if (BIRFORMS == '2550M') { frmType = "frm2550m"; }
	else if (BIRFORMS == '2550Q') { frmType = "frm2550q"; }
	else if (BIRFORMS == '2550Qv2024') { frmType = "frm2550qv2024"; }
	// else if (BIRFORMS == '1702Qv2018C') { frmType = "frm1702qv2018c"; }
	else if (BIRFORMS == '1702Qv2018C') { frmType = "frm1702q"; }
	else if (BIRFORMS == '1700') { frmType = "frm1700"; }
	else if (BIRFORMS == '1700v2018') {frmType = "frm1700"}
	else if (BIRFORMS == '1701') { frmType = "frm1701"; }
	else if (BIRFORMS == '1701A') { frmType = "frm1701A"; }
	else if (BIRFORMS == '1701v2018') { frmType = "frm1701"; }
	else if (BIRFORMS == '1702MX') { frmType = "frm1702MX"; }
	else if (BIRFORMS == '1702MXv2018') { frmType = "frm1702MX"; }
	else if (BIRFORMS == '1702MXv2018C') { frmType = "frm1702MX"; }
	else if (BIRFORMS == '1702RT') { frmType = "frm1702RT"; }
	else if (BIRFORMS == '1702RTv2018') { frmType = "frm1702RT"; }
	else if (BIRFORMS == '1702RTv2018C') { frmType = "frm1702RT"; }
	else if (BIRFORMS == '1702EX') { frmType = "frm1702EX"; }
	else if (BIRFORMS == '1702EXv2018') { frmType = "frm1702EX"; }
	else if (BIRFORMS == '1702EXv2018C') { frmType = "frm1702EX"; }
	else if (BIRFORMS == '1600VTv2018') { frmType = "frm1600VT"; }
	else if (BIRFORMS == '1600PTv2018') { frmType = "frm1600PT"; }
	else if (BIRFORMS == '1707v2018') { frmType = "frm1707"; }
	else if (BIRFORMS == '2552v2018') { frmType = "frm2552"; }
	else if (BIRFORMS == '2200Cv2018') { frmType = "frm2200C"; }
	else if (BIRFORMS == '1701Q') { frmType = "frm1701q"; }
	else if (BIRFORMS == '1701Qv2018') { frmType = "frm1701q"; }
	else if (BIRFORMS == '1702Q') { frmType = "frm1702q"; }
	else if (BIRFORMS == '1702Qv2008C') { frmType = "frm1702q"; }
	else if (BIRFORMS == '2552') { frmType = "frm2552"; }
	else if (BIRFORMS == '2553') { frmType = "frm2553"; }
	else if (BIRFORMS == '1800') { frmType = "frm1800"; }
	else if (BIRFORMS == '1800v2018') { frmType = "frm1800v2018"; }
	else if (BIRFORMS == '0605') { frmType = "frm0605"; }
	else if (BIRFORMS == '0620') { frmType = "frm0620"; }
	else if (BIRFORMS == '0619E') { frmType = "frm0619E"; }
	else if (BIRFORMS == '0619F') { frmType = "frm0619F"; }
	else if (BIRFORMS == '1600WP') { frmType = "frm1600WP"; }
	else if (BIRFORMS == '1604C') { frmType = "frm1604c"; }
	else if (BIRFORMS == '1604CF') { frmType = "frm1604cf"; }
	else if (BIRFORMS == '1604E') { frmType = "frm1604e"; }
	else if (BIRFORMS == '1604Ev2018') { frmType = "frm1604e"; }
	else if (BIRFORMS == '1604F') { frmType = "frm1604f"; }
	else if (BIRFORMS == '1704') { frmType = "frm1704"; }
	else if (BIRFORMS == '1706') { frmType = "frm1706"; }
	else if (BIRFORMS == '1707') { frmType = "frm1707"; }
	else if (BIRFORMS == '1801') { frmType = "frm1801"; }
	else if (BIRFORMS == '1801v2018') { frmType = "frm1801v2018"; }
	else if (BIRFORMS == '2000') { frmType = "frm2000"; }
	else if (BIRFORMS == '2000v2018') { frmType = "frm2000"; }
	else if (BIRFORMS == '2000OT') { frmType = "frm2000OT"; }
	else if (BIRFORMS == '2000OTv2018') { frmType = "frm2000OT"; }
	else if (BIRFORMS == '2200AN') { frmType = "frm2200AN"; }
	else if (BIRFORMS == '2200M') { frmType = "frm2200M"; }
	else if (BIRFORMS == '2200P') { frmType = "frm2200P"; }
	else if (BIRFORMS == '2200A') { frmType = "frm2200A"; }
	else if (BIRFORMS == '2200Av2020') { frmType = "frm2200Av2020"; }
	else if (BIRFORMS == '2200S') { frmType = "frm2200S"; }
	else if (BIRFORMS == '2200T') { frmType = "frm2200T"; }
	else if (BIRFORMS == '2200Tv2020') { frmType = "frm2200Tv2020"; }
	else if (BIRFORMS == '1707A') { frmType = "frm1707A"; }
	else if (BIRFORMS == '1707v2021') { frmType = "frm1707"; }
	else if (BIRFORMS == '1707Av2021') { frmType = "frm1707Av2021"; }
	else if (BIRFORMS == '2200Pv2020') { frmType = "frm2200P"; }
	else if (BIRFORMS == '2200Mv2018') { frmType = "frm2200M"; }
	else if (BIRFORMS == '2550DS') { frmType = "frm2550ds"; }
	else if (BIRFORMS == '1701MS') { frmType = "frm1701MS"; }
    
	if (document.getElementById('txtFinalFlag').value == "2") {
		document.getElementById(frmType + ':btnFinalCopy').disabled = false;
	}
	else if (document.getElementById('txtFinalFlag').value == "1") {
		gIsReadOnly = true;
		document.getElementById(frmType + ':btnFinalCopy').disabled = true;

	}
	else if (document.getElementById('txtFinalFlag').value == "3" && gIsReadOnly) {

		document.getElementById(frmType + ':btnFinalCopy').disabled = false;

	}
	else if (document.getElementById('txtFinalFlag').value == "0" && gIsReadOnly) {

		document.getElementById(frmType + ':btnFinalCopy').disabled = false;
		document.getElementById('txtFinalFlag').value = "2"
		//gIsReadOnly = false;
	}
}
function getFTPFolderName(BIRFORMS) {
	var frmType = "";
	if (BIRFORMS.indexOf('-1601C-') > -1) { frmType = "1601C"; }
	else if (BIRFORMS.indexOf('-1601Cv2018-') > -1) { frmType = "1601Cv2018"; }
	else if (BIRFORMS.indexOf('-1601E-') > -1) { frmType = "1601E"; }
	else if (BIRFORMS.indexOf('-1601EQ-') > -1) { frmType = "1601EQ"; }
	else if (BIRFORMS.indexOf('-1601F-') > -1) { frmType = "1601F"; }
	else if (BIRFORMS.indexOf('-1601FQ-') > -1) { frmType = "1601FQ"; }
	else if (BIRFORMS.indexOf('-1600-') > -1) { frmType = "1600"; }
	else if (BIRFORMS.indexOf('-1603-') > -1) { frmType = "1603"; }
	else if (BIRFORMS.indexOf('-1603Qv2018-') > -1) { frmType = "1603Qv2018"; }
	else if (BIRFORMS.indexOf('-1602-') > -1) { frmType = "1602"; }
	else if (BIRFORMS.indexOf('-1602Qv2018-') > -1) { frmType = "1602Qv2018"; }
	else if (BIRFORMS.indexOf('-1606-') > -1) { frmType = "1606"; }
	else if (BIRFORMS.indexOf('-1621-') > -1) { frmType = "1621"; }
	else if (BIRFORMS.indexOf('-2551M-') > -1) { frmType = "2551M"; }
	else if (BIRFORMS.indexOf('-2551-') > -1) { frmType = "2551Q"; }
	else if (BIRFORMS.indexOf('-2551Qv2018-') > -1) { frmType = "2551Qv2018"; }
	else if (BIRFORMS.indexOf('-2550M-') > -1) { frmType = "2550M"; }
	else if (BIRFORMS.indexOf('-2550Q-') > -1) { frmType = "2550Q"; }
	else if (BIRFORMS.indexOf('-2550Qv2024-') > -1) { frmType = "2550Qv2024"; }
	else if (BIRFORMS.indexOf('-1702Qv2018C-') > -1) { frmType = "1702Qv2018C"; }
	else if (BIRFORMS.indexOf('-1700-') > -1) { frmType = "1700v2013"; }
	else if (BIRFORMS.indexOf('-1700v2018-') > -1) { frmType = "1700v2018"; }
	else if (BIRFORMS.indexOf('-1701-') > -1) { frmType = "1701v2013"; }
	else if (BIRFORMS.indexOf('-1701A-') > -1) { frmType = "1701A"; }
	else if (BIRFORMS.indexOf('-1701v2018-') > -1) { frmType = "1701v2018"; }
	else if (BIRFORMS.indexOf('-1702MX-') > -1) { frmType = "1702MXv2013"; }
	else if (BIRFORMS.indexOf('-1702MXv2018-') > -1) { frmType = "1702MXv2018"; }
	else if (BIRFORMS.indexOf('-1702MXv2018C-') > -1) { frmType = "1702MXv2018C"; }
	else if (BIRFORMS.indexOf('-1702RT-') > -1) { frmType = "1702RTv2013"; }
	else if (BIRFORMS.indexOf('-1702RTv2018-') > -1) { frmType = "1702RTv2018"; }
	else if (BIRFORMS.indexOf('-1702RTv2018C-') > -1) { frmType = "1702RTv2018C"; }
	else if (BIRFORMS.indexOf('-1702EX-') > -1) { frmType = "1702EXv2013"; }
	else if (BIRFORMS.indexOf('-1702EXv2018-') > -1) { frmType = "1702EXv2018"; }
	else if (BIRFORMS.indexOf('-1702EXv2018C-') > -1) { frmType = "1702EXv2018C"; }
	else if (BIRFORMS.indexOf('-1600VTv2018-') > -1) { frmType = "1600VTv2018"; }
	else if (BIRFORMS.indexOf('-1600PTv2018-') > -1) { frmType = "1600PTv2018"; }
	else if (BIRFORMS.indexOf('-1707v2018-') > -1) { frmType = "1707v2018"; }
	else if (BIRFORMS.indexOf('-2552v2018-') > -1) { frmType = "2552v2018"; }
	else if (BIRFORMS.indexOf('-2200Cv2018-') > -1) { frmType = "2200Cv2018"; }
	else if (BIRFORMS.indexOf('-1701Q-') > -1) { frmType = "1701Qv2008"; }
	else if (BIRFORMS.indexOf('-1701Qv2018-') > -1) { frmType = "1701Qv2018"; }
	else if (BIRFORMS.indexOf('-1702Q-') > -1) { frmType = "1702Qv2008"; }
	else if (BIRFORMS.indexOf('-2552-') > -1) { frmType = "2552"; }
	else if (BIRFORMS.indexOf('-2553-') > -1) { frmType = "2553"; }
	else if (BIRFORMS.indexOf('-1800-') > -1) { frmType = "1800"; }
	else if (BIRFORMS.indexOf('-1800v2018-') > -1) { frmType = "1800v2018"; }
	else if (BIRFORMS.indexOf('-0605-') > -1) { frmType = "0605"; }
	else if (BIRFORMS.indexOf('-0620-') > -1) { frmType = "0620"; }
	else if (BIRFORMS.indexOf('-0619E-') > -1) { frmType = "0619E"; }
	else if (BIRFORMS.indexOf('-0619F-') > -1) { frmType = "0619F"; }
	else if (BIRFORMS.indexOf('-1600WP-') > -1) { frmType = "1600WP"; }
	else if (BIRFORMS.indexOf('-1604C-') > -1) { frmType = "1604C"; }
	else if (BIRFORMS.indexOf('-1604CF-') > -1) { frmType = "1604CF"; }
	else if (BIRFORMS.indexOf('-1604E-') > -1) { frmType = "1604E"; }
	else if (BIRFORMS.indexOf('-1604Ev2018-') > -1) { frmType = "1604Ev2018"; }
	else if (BIRFORMS.indexOf('-1604F-') > -1) { frmType = "1604F"; }
	else if (BIRFORMS.indexOf('-1704-') > -1) { frmType = "1704"; }
	else if (BIRFORMS.indexOf('-1706-') > -1) { frmType = "1706"; }
	else if (BIRFORMS.indexOf('-1707-') > -1) { frmType = "1707"; }
	else if (BIRFORMS.indexOf('-1801-') > -1) { frmType = "1801"; }
	else if (BIRFORMS.indexOf('-1801v2018-') > -1) { frmType = "1801v2018"; }
	else if (BIRFORMS.indexOf('-2000-') > -1) { frmType = "2000"; }
	else if (BIRFORMS.indexOf('-2000v2018-') > -1) { frmType = "2000v2018"; }
	else if (BIRFORMS.indexOf('-2000OT-') > -1) { frmType = "2000OT"; }
	else if (BIRFORMS.indexOf('-2000OTv2018-') > -1) { frmType = "2000OTv2018"; }
	else if (BIRFORMS.indexOf('-2200AN-') > -1) { frmType = "2200AN"; }
	else if (BIRFORMS.indexOf('-2200M-') > -1) { frmType = "2200M"; }
	else if (BIRFORMS.indexOf('-2200P-') > -1) { frmType = "2200P"; }
	else if (BIRFORMS.indexOf('-2200A-') > -1) { frmType = "2200Av2013"; }
	else if (BIRFORMS.indexOf('-2200S-') > -1) { frmType = "2200S"; }
	else if (BIRFORMS.indexOf('-2200T-') > -1) { frmType = "2200Tv2013"; }
	else if (BIRFORMS.indexOf('-2200Tv2020-') > -1) { frmType = "2200Tv2020"; }
	else if (BIRFORMS.indexOf('-1707A-') > -1) { frmType = "1707Av2015"; }
	else if (BIRFORMS.indexOf('-1707Av2021-') > -1) { frmType = "1707Av2021"; }
	else if (BIRFORMS.indexOf('-1707v2021-') > -1) { frmType = "1707v2021"; }
	else if (BIRFORMS.indexOf('-2200Pv2020-') > -1) { frmType = "2200Pv2020"; }
	else if (BIRFORMS.indexOf('-2200Mv2018-') > -1) { frmType = "2200Mv2018"; }
	else if (BIRFORMS.indexOf('-1701MS-') > -1) { frmType = "1701MS"; }

	return frmType;
}


function disableDropDown(param) {
	if (param == "before") {
		$('#formMenu').hide();
		$('#header').hide();
		// document.getElementById('drpFile').disabled = true;
		// document.getElementById('drpHelp').disabled = true;
	}
	else if (param == "after") {
		$('#formMenu').show();
		$('#header').show();
		// document.getElementById('drpFile').disabled = false;
		// document.getElementById('drpHelp').disabled = false;
	}
}

function clearFileNameWOemail() {
	var fileSys = new ActiveXObject("Scripting.FileSystemObject");
	var Fo = new ActiveXObject("Scripting.FileSystemObject");

	var fullPath = fileSys.GetAbsolutePathName('cFTPSend.exe');
	var mainFolder = fullPath.split('cFTPSend.exe')[0];
	var Rdofolder = mainFolder + "\IAF_RDO_Copy";
	var fileNameInRdoFolder = "";

	var fsObj = new Enumerator(Fo.GetFolder(Rdofolder).Files);

	for (i = 0; !fsObj.atEnd(); fsObj.moveNext()) {
		if (fsObj.item().Size > 0) {

			fileNameInRdoFolder = fsObj.item().Name;

			if (fileNameInRdoFolder.indexOf("#") == -1) {
				Fo.DeleteFile(Rdofolder + "\\" + fileNameInRdoFolder, true);
			}
		}
	}
}
function SetFinalFlagTo0() {
	if (document.getElementById("txtFinalFlag").value == "2") {
		document.getElementById("txtFinalFlag").value = "3";
	}
	else {
		document.getElementById("txtFinalFlag").value = "0";
	}
}


function getDisplayPageURL(redirectURL, fkID) {
	var urlArr = redirectURL.split("/");
	var nodeUsed = "prod1";
	var site = "PROD";

	for (var i = 0; i <= urlArr.length; i++) {
		if (urlArr[i] != undefined && urlArr[i].indexOf("prod") != -1) {
			nodeUsed = urlArr[i];
		}
	}

	redirectURL = "https://ebirforms.bir.gov.ph:443/" + nodeUsed + "/m/main.jsp?pageId=357808&id=" + fkID;

	if (site == "UAT") { //UAT URL
		redirectURL = "https://ebirforms.bir.gov.ph:443/" + nodeUsed + "/m/main.jsp?pageId=635172&id=" + fkID;
	} else if (site == "MASTER") { //LOCAL VM URL
		redirectURL = "http://220.5.13.94:8080/master/m/main.jsp?pageId=10937&id=" + fkID;
	}

	return redirectURL;
}

function geteBIRFormsOnlineLoginURL() {
	var site = "PROD";
	//PROD URL
	var loginURL = 'https://ebirforms.bir.gov.ph:443/prod1/portal/portal.jsp?c=7727&p=42602&g=37242';

	if (site == "UAT") { //UAT URL
		loginURL = 'https://ebirforms.bir.gov.ph:443/prod1/portal/portal.jsp?c=482019&p=493732&g=639970';
	} else if (site == "MASTER") { //LOCAL VM URL
		loginURL = 'http://220.5.13.94:8080/master/portal/portal.jsp?c=1&p=16397&g=16110';
	}

	return loginURL;
}

function getURL() {
	var site = "PROD";
	var returnURL = (site == "PROD" || site == "UAT") ? 'https://ebirforms.bir.gov.ph:443' : 'http://220.5.13.94:8080';

	return returnURL;
}
