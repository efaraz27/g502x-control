fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=packaging/g502x-control.ico");
        winresource::WindowsResource::new()
            .set_icon("packaging/g502x-control.ico")
            .compile()
            .expect("Could not embed Windows application resources");
    }
}
