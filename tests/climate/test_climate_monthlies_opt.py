import numpy as np
from wepppyo3.climate import rust_cli_calculate_p_annual_monthlies, calculate_monthlies
from wepppy.climates.cligen import ClimateFile

if __name__ == "__main__":
    from pprint import pprint
    import time
    
    src_fn = 'stochastic.cli'
    
    iterations = 10

    # Measure rust-based calculation performance
    start = time.perf_counter()
    for _ in range(iterations):
        monthlies = calculate_monthlies(src_fn)
    rust_elapsed = time.perf_counter() - start
    print("Rust method total time:", rust_elapsed)
    print("Rust method average time:", rust_elapsed / iterations)
    #pprint(monthlies)

    # Measure pure Python-based calculation performance
    start = time.perf_counter()
    for _ in range(iterations):
        cli = ClimateFile(src_fn)
        monthlies2 = cli.calc_monthlies()
    python_elapsed = time.perf_counter() - start
    print("Pure Python method total time:", python_elapsed)
    print("Pure Python method average time:", python_elapsed / iterations)
    #pprint(monthlies2)


