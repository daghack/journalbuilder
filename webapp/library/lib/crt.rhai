// Remainders and the numbers that leave them: the Chinese remainder
// theorem, for locks whose answer is when several cycles come round
// together (star_alignment).
// import "lib/crt" as crt;

// The greatest number dividing both.
fn gcd(a, b) {
	while b != 0 {
		let rest = a % b;
		a = b;
		b = rest;
	}
	if a < 0 { -a } else { a }
}

// The least number both divide; of a list, the least all of it divides.
fn lcm(a, b) {
	a / gcd(a, b) * b
}

fn lcm_of(numbers) {
	numbers.reduce(|all, n| lcm(all, n), 1)
}

// [g, x, y]: the greatest number dividing a and b, and whole numbers
// with a x + b y = g.
fn bezout(a, b) {
	let old = [a, 1, 0];
	let now = [b, 0, 1];
	while now[0] != 0 {
		let q = old[0] / now[0];
		let next = [old[0] - q * now[0], old[1] - q * now[1], old[2] - q * now[2]];
		old = now;
		now = next;
	}
	old
}

// The numbers leaving remainders[k] when divided by moduli[k], for every
// k: #{ value, modulus }, the least of them that's 0 or more, and how far
// apart they come (the least number all the moduli divide). The moduli
// needn't be coprime; throws if no number leaves all the remainders.
fn solve(remainders, moduli) {
	let value = 0;
	let modulus = 1;
	for k in 0..moduli.len() {
		let m = moduli[k];
		let r = ((remainders[k] % m) + m) % m;
		let found = bezout(modulus, m);
		let g = found[0];
		if (r - value) % g != 0 {
			throw `no number leaves ${value} over ${modulus} and ${r} over ${m}`;
		}
		let whole = modulus / g * m;
		let steps = ((r - value) / g * found[1]) % (m / g);
		value = ((value + modulus * steps) % whole + whole) % whole;
		modulus = whole;
	}
	#{ value: value, modulus: modulus }
}
