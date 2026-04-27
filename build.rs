fn main() {
    // Tell cargo to look for libraries in the libmpv directory
    println!("cargo:rustc-link-search=native=libmpv");
}
