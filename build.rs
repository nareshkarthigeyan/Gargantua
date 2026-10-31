fn main() {
    println!("cargo:rerun-if-changed=assets/icons/gargantua.ico");
    #[cfg(windows)]
    {
        winresource::WindowsResource::new()
            .set_icon("assets/icons/gargantua.ico")
            .set("ProductName", "Gargantua")
            .set("FileDescription", "Gargantua — A Scripted Descent")
            .set("LegalCopyright", "MIT License")
            .compile()
            .expect("Failed to embed Windows application icon");
    }
}
