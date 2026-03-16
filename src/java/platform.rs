use std::path::PathBuf;

/// Get platform-specific Java search paths
pub fn get_java_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Runner-specific installation directory
    if let Some(data_dir) = dirs::data_local_dir() {
        paths.push(data_dir.join("hytale-runner").join("java"));
    }

    #[cfg(target_os = "windows")]
    {
        paths.extend(get_windows_paths());
    }

    #[cfg(target_os = "linux")]
    {
        paths.extend(get_linux_paths());
    }

    #[cfg(target_os = "macos")]
    {
        paths.extend(get_macos_paths());
    }

    paths
}

#[cfg(target_os = "windows")]
fn get_windows_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Common Windows Java installation paths
    paths.push(PathBuf::from(r"C:\Program Files\Java"));
    paths.push(PathBuf::from(r"C:\Program Files\Eclipse Adoptium"));
    paths.push(PathBuf::from(r"C:\Program Files\Microsoft"));

    // User-local installations
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        paths.push(
            PathBuf::from(&local_app_data)
                .join("Programs")
                .join("Eclipse Adoptium"),
        );
    }

    paths
}

#[cfg(target_os = "linux")]
fn get_linux_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Common Linux Java installation paths
    paths.push(PathBuf::from("/usr/lib/jvm"));
    paths.push(PathBuf::from("/opt/java"));
    paths.push(PathBuf::from("/usr/java"));

    // User-local installations
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".sdkman").join("candidates").join("java"));
        paths.push(home.join(".jabba").join("jdk"));
        paths.push(home.join(".jenv").join("versions"));
    }

    paths
}

#[cfg(target_os = "macos")]
fn get_macos_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // System-wide Java installations
    paths.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));

    // User-local installations
    if let Some(home) = dirs::home_dir() {
        paths.push(
            home.join("Library")
                .join("Java")
                .join("JavaVirtualMachines"),
        );
        paths.push(home.join(".sdkman").join("candidates").join("java"));
        paths.push(home.join(".jabba").join("jdk"));
    }

    paths
}

/// Get the Java executable name for the current platform
pub fn java_executable_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "java.exe"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "java"
    }
}

/// Get the OS name for Adoptium API
pub fn adoptium_os() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(target_os = "macos")]
    {
        "mac"
    }
}

/// Get the architecture name for Adoptium API
pub fn adoptium_arch() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        "x64"
    }
    #[cfg(target_arch = "aarch64")]
    {
        "aarch64"
    }
    #[cfg(target_arch = "x86")]
    {
        "x32"
    }
}

/// Get the archive extension for the current platform
pub fn archive_extension() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "zip"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "tar.gz"
    }
}

/// Get the hytale-runner Java installation directory
pub fn runner_java_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("hytale-runner").join("java"))
}
