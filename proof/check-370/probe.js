// Probe evaluation: the OFFICIAL rounding paths of the eBIRForms runtime,
// copied verbatim from the mirrored official runtime source (kapitanluffy/ebirforms):
// js/string-util.js round() / formatCurrency() / NumWithComma / isAmountWithinAllowedPrecision,
// and the 2551Q HTA blockletter() (the toFixed path the rules summary attributes to money boxes).

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

function blockletter(e) {
	var number = parseFloat(e.value).toFixed(2);
	if (isNaN(number)) {
		var zero = 0;
		e.value = parseFloat(zero).toFixed(2);
	} else {
		e.value = '' + number;
	}
}

// computeATC(1) for the first Schedule 1 row at the PT010 3% rate:
function computeATC(amountBoxValue) {
    var taxDue = NumWithComma(amountBoxValue) * (3 / 100);
    return formatCurrency(taxDue);
}

const probes = ["2000.005", "0.005", "1.005", "2.675", "1234.565", "99999999.995", "1,000.005", " 12.345 ", "100.0049"];

console.log("probe\tblur-round(box shows)\ttaxDue@3%\tblockletter-toFixed(counterfactual)");
for (const p of probes) {
    const box = { value: p };
    round(box, 2);
    const shown = box.value;
    const tax = computeATC(shown);
    const cb = { value: p };
    blockletter(cb);
    console.log(JSON.stringify(p) + "\t" + shown + "\t" + tax + "\t" + cb.value);
}
