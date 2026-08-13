"""Exact integer arithmetic for the determinant screen.

Hard rule for this package: no floating point, anywhere. Every quantity that
reaches a conclusion is an int or a fractions.Fraction. There is no tolerance
parameter and no rounding, because the questions being asked ("is this ratio
exactly a sixth power of a rational?") have exact answers and a float would
silently invent one.
"""

from __future__ import annotations

import random
from fractions import Fraction

# ---------------------------------------------------------------- roots


def iroot(n: int, k: int) -> int:
    """Floor of the k-th root of n >= 0, exactly (integer Newton iteration)."""
    if k < 1:
        raise ValueError("k must be >= 1")
    if n < 0:
        raise ValueError("iroot expects n >= 0")
    if n < 2:
        return n
    x = 1 << ((n.bit_length() + k - 1) // k)
    while True:
        y = ((k - 1) * x + n // x ** (k - 1)) // k
        if y >= x:
            return x
        x = y


def is_perfect_power(n: int, k: int) -> bool:
    """True iff n is an exact k-th power of a non-negative integer."""
    if n < 0:
        return False
    return iroot(n, k) ** k == n


def exact_root(n: int, k: int) -> int | None:
    """The exact k-th root of n if it exists, else None."""
    if n < 0:
        return None
    r = iroot(n, k)
    return r if r**k == n else None


# ---------------------------------------------------------- factorisation


def _is_probable_prime(n: int) -> bool:
    """Deterministic Miller-Rabin for n < 3.3e24 with this witness set."""
    if n < 2:
        return False
    small = (2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37)
    for p in small:
        if n % p == 0:
            return n == p
    d, s = n - 1, 0
    while d % 2 == 0:
        d //= 2
        s += 1
    for a in small:
        x = pow(a, d, n)
        if x == 1 or x == n - 1:
            continue
        for _ in range(s - 1):
            x = x * x % n
            if x == n - 1:
                break
        else:
            return False
    return True


def _pollard_rho(n: int, rng: random.Random) -> int:
    """A non-trivial factor of composite n."""
    if n % 2 == 0:
        return 2
    while True:
        c = rng.randrange(1, n)
        x = y = rng.randrange(0, n)
        d = 1
        while d == 1:
            x = (x * x + c) % n
            y = (y * y + c) % n
            y = (y * y + c) % n
            d = _gcd(abs(x - y), n)
        if d != n:
            return d


def _gcd(a: int, b: int) -> int:
    while b:
        a, b = b, a % b
    return a


def factorise(n: int) -> dict[int, int]:
    """Prime factorisation of |n| as {prime: exponent}. factorise(0) raises.

    Trial division by small primes, then Pollard rho. The invariants in this
    project are <= 8 digits so trial division alone suffices; rho is here so
    the tool stays correct if it is ever pointed at larger data.
    """
    n = abs(n)
    if n == 0:
        raise ValueError("0 has no factorisation")
    out: dict[int, int] = {}
    for p in (2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47):
        while n % p == 0:
            out[p] = out.get(p, 0) + 1
            n //= p
    if n == 1:
        return out
    d = 53
    while d * d <= n and d < 1_000_000:
        while n % d == 0:
            out[d] = out.get(d, 0) + 1
            n //= d
        d += 2
    if n == 1:
        return out
    rng = random.Random(0)  # fixed seed: identical input -> identical run
    stack = [n]
    while stack:
        m = stack.pop()
        if m == 1:
            continue
        if _is_probable_prime(m):
            out[m] = out.get(m, 0) + 1
            continue
        f = _pollard_rho(m, rng)
        stack.append(f)
        stack.append(m // f)
    return out


# ------------------------------------------------------ residue signature


def residue_signature(n: int, w: int) -> tuple:
    """The class of n modulo w-th powers of rationals.

    Two non-zero integers a, b satisfy  a/b = q**w  for some rational q
    if and only if residue_signature(a, w) == residue_signature(b, w).

    Returned as (sign, ((p, e mod w), ...)) with zero residues dropped and
    primes sorted, so the value is hashable and canonical.

    The sign matters only for even w. For even w, q**w > 0 forces a and b to
    share a sign, so the sign is part of the class. For odd w, -1 = (-1)**w is
    itself a w-th power, so sign carries no information and is normalised away
    -- folding it in regardless would wrongly separate a from -a.
    """
    if n == 0:
        raise ValueError("the signature of 0 is not defined")
    sign = (1 if n > 0 else -1) if w % 2 == 0 else 1
    residues = tuple(
        sorted((p, e % w) for p, e in factorise(n).items() if e % w != 0)
    )
    return (sign, residues)


def ratio_is_wth_power(a: int, b: int, w: int) -> bool:
    """True iff a/b is exactly the w-th power of a rational. No factorisation.

    This is the independent check on the signature grouping: it reduces a/b to
    lowest terms and asks whether numerator and denominator are each exact
    w-th powers of integers.
    """
    if b == 0:
        raise ValueError("b must be non-zero")
    if a == 0:
        return False
    r = Fraction(a, b)
    num, den = r.numerator, r.denominator
    if num < 0:
        if w % 2 == 0:
            return False
        return is_perfect_power(-num, w) and is_perfect_power(den, w)
    return is_perfect_power(num, w) and is_perfect_power(den, w)


def wth_root_of_ratio(a: int, b: int, w: int) -> Fraction | None:
    """The exact positive w-th root of a/b as a Fraction, or None if not exact."""
    if not ratio_is_wth_power(a, b, w):
        return None
    r = Fraction(a, b)
    num, den = abs(r.numerator), r.denominator
    rn, rd = exact_root(num, w), exact_root(den, w)
    if rn is None or rd is None:
        return None
    return Fraction(rn, rd)
