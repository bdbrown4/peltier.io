/* hotpath kernel-lane demonstration (SPEC §13).
 *
 * A single-precision matrix multiply in two implementations that are
 * mathematically identical but numerically different because they sum the
 * inner products in a different ORDER:
 *
 *   matmul_ref — naive i,j,k with a single sequential accumulator. The
 *                reference oracle.
 *   matmul_opt — eight independent accumulators over the k-reduction,
 *                tree-combined at the end. This breaks the float-add
 *                dependency chain (hiding ~4-cycle add latency behind
 *                instruction-level parallelism, so it is faster) AND sums
 *                the inner product in a different ORDER, so results differ
 *                in the last few ULPs — bit-identical is impossible by
 *                construction.
 *
 * This is the crux of the kernel lane: a real optimization here CANNOT be
 * gated by byte-identical golden replay (the low bits differ by design);
 * it needs a floating-point *tolerance* gate that accepts last-ULP
 * reordering while still catching a genuine wrong result. The reference
 * implementation is the oracle; the candidate is differential-tested
 * against it within a declared tolerance. On a GPU this same shape holds
 * (a Triton/CUDA kernel vs a reference kernel) — only the timer and the
 * hardware change.
 *
 *   kernel <n> emit  ref|opt              # print the result matrix
 *   kernel <n> check <abs-tol> <rel-tol>  # in-process differential test
 *   kernel <n> run   ref|opt              # one matmul, no output (for A/B)
 *   kernel <n> bench ref|opt <reps>       # time one implementation
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <time.h>

#define BS 64 /* block size */

/* Deterministic f32 fill — xorshift, identical on every machine. */
static void fill(float *m, long n, unsigned seed) {
    unsigned s = seed ? seed : 1;
    for (long i = 0; i < n * n; i++) {
        s ^= s << 13; s ^= s >> 17; s ^= s << 5;
        /* in [-1, 1) */
        m[i] = (float)((double)s / 2147483648.0 - 1.0);
    }
}

static void matmul_ref(const float *A, const float *B, float *C, long n) {
    for (long i = 0; i < n; i++)
        for (long j = 0; j < n; j++) {
            float acc = 0.0f;
            for (long k = 0; k < n; k++)
                acc += A[i * n + k] * B[k * n + j];
            C[i * n + j] = acc;
        }
}

static void matmul_opt(const float *A, const float *B, float *C, long n) {
    /* Transpose B so the inner reduction walks BT[j] contiguously instead
     * of striding B by n (the cache win), and use eight independent
     * accumulators (the ILP win, which also REORDERS the reduction). Both
     * levers together are a robust speedup AND make bit-identical output
     * impossible — the kernel lane in one function. */
    float *BT = malloc((size_t)n * n * sizeof(float));
    if (!BT) {
        matmul_ref(A, B, C, n);
        return;
    }
    for (long i = 0; i < n; i++)
        for (long j = 0; j < n; j++)
            BT[j * n + i] = B[i * n + j];
    for (long i = 0; i < n; i++) {
        const float *a = &A[i * n];
        for (long j = 0; j < n; j++) {
            const float *b = &BT[j * n];
            float acc[8] = {0, 0, 0, 0, 0, 0, 0, 0};
            long k = 0;
            for (; k + 8 <= n; k += 8)
                for (int r = 0; r < 8; r++)
                    acc[r] += a[k + r] * b[k + r];
            float s = ((acc[0] + acc[1]) + (acc[2] + acc[3]))
                    + ((acc[4] + acc[5]) + (acc[6] + acc[7]));
            for (; k < n; k++)
                s += a[k] * b[k];
            C[i * n + j] = s;
        }
    }
    free(BT);
}

static double now_s(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (double)t.tv_sec + (double)t.tv_nsec / 1e9;
}

int main(int argc, char **argv) {
    if (argc < 3) {
        fprintf(stderr, "usage: %s <n> emit ref|opt | check <abs-tol> <rel-tol> | run ref|opt | bench ref|opt <reps>\n", argv[0]);
        return 2;
    }
    long n = atol(argv[1]);
    if (n < 1) n = 512;
    const char *mode = argv[2];

    float *A = malloc((size_t)n * n * sizeof(float));
    float *B = malloc((size_t)n * n * sizeof(float));
    float *C = malloc((size_t)n * n * sizeof(float));
    float *R = malloc((size_t)n * n * sizeof(float));
    if (!A || !B || !C || !R) return 2;
    fill(A, n, 0x9E3779B1u);
    fill(B, n, 0x85EBCA77u);

    if (strcmp(mode, "emit") == 0) {
        int blocked = (argc > 3 && strcmp(argv[3], "opt") == 0);
        if (blocked) matmul_opt(A, B, C, n);
        else matmul_ref(A, B, C, n);
        for (long i = 0; i < n * n; i++) printf("%.9g\n", C[i]);
        return 0;
    }

    if (strcmp(mode, "check") == 0) {
        /* Combined abs+rel tolerance — the standard for FP comparison, and
         * what the diff-test policy uses. Rel-only is wrong here: result
         * elements near zero (catastrophic cancellation) have meaningless
         * relative error, so an abs floor is required. */
        double abs_tol = argc > 3 ? atof(argv[3]) : 1e-4;
        double rel_tol = argc > 4 ? atof(argv[4]) : 1e-3;
        matmul_ref(A, B, R, n);
        matmul_opt(A, B, C, n);
        double max_abs = 0.0, max_rel = 0.0, worst_excess = -1e30;
        for (long i = 0; i < n * n; i++) {
            double d = fabs((double)C[i] - (double)R[i]);
            double r = d / (fabs((double)R[i]) + 1e-30);
            double tol = abs_tol + rel_tol * fabs((double)R[i]);
            if (d > max_abs) max_abs = d;
            if (r > max_rel) max_rel = r;
            if (d - tol > worst_excess) worst_excess = d - tol;
        }
        int pass = worst_excess <= 0.0;
        printf("kernel differential test (opt vs reference):\n");
        printf("  max_abs_err = %.3e\n", max_abs);
        printf("  max_rel_err = %.3e (dominated by near-zero elements; see abs floor)\n", max_rel);
        printf("  tolerance   = abs %.3e + rel %.3e * |ref|\n", abs_tol, rel_tol);
        printf("  byte_identical   = NO (accumulation reordered by design)\n");
        printf("  within_tolerance = %s\n", pass ? "YES" : "NO");
        printf("  verdict = %s\n", pass ? "EQUIVALENT" : "DIVERGENT");
        return pass ? 0 : 1;
    }

    /* One matmul, no output — for bench-runner's interleaved A/B (external
     * wall-clock timing with a bootstrap CI, like every other target). */
    if (strcmp(mode, "run") == 0) {
        if (strcmp(argv[3], "opt") == 0) matmul_opt(A, B, C, n);
        else matmul_ref(A, B, C, n);
        /* touch C so the compiler cannot elide the work */
        volatile float sink = C[n * n - 1];
        (void)sink;
        return 0;
    }

    if (strcmp(mode, "bench") == 0) {
        int use_opt = strcmp(argv[3], "opt") == 0;
        long reps = argc > 4 ? atol(argv[4]) : 5;
        if (use_opt) matmul_opt(A, B, C, n); else matmul_ref(A, B, C, n);
        double best = 1e30;
        for (long r = 0; r < reps; r++) {
            double t0 = now_s();
            if (use_opt) matmul_opt(A, B, C, n); else matmul_ref(A, B, C, n);
            double dt = now_s() - t0;
            if (dt < best) best = dt;
        }
        printf("%.6f\n", best);
        return 0;
    }

    fprintf(stderr, "unknown mode: %s\n", mode);
    return 2;
}
