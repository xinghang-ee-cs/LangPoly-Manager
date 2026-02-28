use super::*;

impl PythonInstaller {
    pub(super) fn try_adopt_existing_installation(&self, version: &str) -> Result<bool> {
        self.try_adopt_existing_installation_with_progress(version, None)
    }

    pub(super) fn try_adopt_existing_installation_with_progress(
        &self,
        version: &str,
        progress: Option<&ProgressBar>,
    ) -> Result<bool> {
        let Some(existing_dir) = self.find_existing_system_python_dir(version)? else {
            return Ok(false);
        };

        let install_dir = self.get_install_dir(version);
        if install_dir.exists() {
            std::fs::remove_dir_all(&install_dir).with_context(|| {
                format!(
                    "Failed to remove existing managed install dir before adoption: {}",
                    install_dir.display()
                )
            })?;
        }
        std::fs::create_dir_all(&install_dir).with_context(|| {
            format!(
                "Failed to create managed install dir before adoption: {}",
                install_dir.display()
            )
        })?;

        if let Some(pb) = progress {
            pb.set_position(20);
            pb.set_message("🔍 正在分析系统 Python 文件清单...");
        }

        let plan = Self::build_copy_plan(&existing_dir, progress).with_context(|| {
            format!(
                "Failed to analyze Python installation layout before import: {}",
                existing_dir.display()
            )
        })?;
        let mut status = DirectoryCopyStatus::default();

        if let Some(pb) = progress {
            pb.set_position(35);
            pb.set_message(format!(
                "📂 正在导入系统 Python（共 {} 个文件）...",
                plan.file_count
            ));
        }

        Self::copy_directory_contents_with_progress(
            &existing_dir,
            &install_dir,
            &plan,
            &mut status,
            progress,
        )
        .with_context(|| {
            format!(
                "Failed to import existing Python installation from '{}' to '{}'",
                existing_dir.display(),
                install_dir.display()
            )
        })?;
        if let Some(pb) = progress {
            pb.set_position(97);
            pb.set_message("✅ 正在完成导入收尾...");
        }

        println!(
            "检测到系统已安装 Python {}（{}），已导入到 MeetAI 目录。",
            version,
            existing_dir.display()
        );

        Ok(true)
    }

    #[cfg(test)]
    pub(super) fn copy_directory_contents(source_dir: &Path, target_dir: &Path) -> Result<()> {
        let plan = Self::build_copy_plan(source_dir, None)?;
        let mut status = DirectoryCopyStatus::default();
        Self::copy_directory_contents_with_progress(
            source_dir,
            target_dir,
            &plan,
            &mut status,
            None,
        )
    }

    pub(super) fn build_copy_plan(
        source_dir: &Path,
        progress: Option<&ProgressBar>,
    ) -> Result<DirectoryCopyPlan> {
        let mut plan = DirectoryCopyPlan::default();
        let mut scanned_files = 0u64;
        Self::collect_copy_plan_recursive(source_dir, &mut plan, &mut scanned_files, progress)?;
        Ok(plan)
    }

    pub(super) fn collect_copy_plan_recursive(
        source_dir: &Path,
        plan: &mut DirectoryCopyPlan,
        scanned_files: &mut u64,
        progress: Option<&ProgressBar>,
    ) -> Result<()> {
        if !source_dir.exists() {
            anyhow::bail!(
                "Source directory for copy does not exist: {}",
                source_dir.display()
            );
        }

        for entry in std::fs::read_dir(source_dir)
            .with_context(|| format!("Failed to read source dir: {}", source_dir.display()))?
        {
            let entry = entry
                .with_context(|| format!("Failed to read entry in {}", source_dir.display()))?;
            let source_path = entry.path();
            let metadata = std::fs::symlink_metadata(&source_path).with_context(|| {
                format!(
                    "Failed to read source metadata (without following symlink): {}",
                    source_path.display()
                )
            })?;
            if Self::is_symlink_or_reparse_point(&metadata) {
                anyhow::bail!(
                    "Refusing to import symbolic link/reparse point: {}",
                    source_path.display()
                );
            }

            if metadata.is_dir() {
                Self::collect_copy_plan_recursive(&source_path, plan, scanned_files, progress)?;
            } else if metadata.is_file() {
                plan.file_count += 1;
                plan.total_bytes += metadata.len();
                if plan.file_count > MAX_ADOPT_FILE_COUNT {
                    anyhow::bail!(
                        "Refusing to import Python installation with too many files ({} > {}): {}",
                        plan.file_count,
                        MAX_ADOPT_FILE_COUNT,
                        source_dir.display()
                    );
                }
                if plan.total_bytes > MAX_ADOPT_TOTAL_BYTES {
                    anyhow::bail!(
                        "Refusing to import Python installation larger than limit ({} bytes > {} bytes): {}",
                        plan.total_bytes,
                        MAX_ADOPT_TOTAL_BYTES,
                        source_dir.display()
                    );
                }
                *scanned_files += 1;
                if let Some(pb) = progress {
                    if (*scanned_files).is_multiple_of(40) {
                        let scanned_step = (*scanned_files / 40).min(14);
                        let next_pos = (20 + scanned_step).min(34);
                        if next_pos > pb.position() {
                            pb.set_position(next_pos);
                        }
                        pb.set_message(format!(
                            "🔍 正在分析系统 Python 文件清单（已扫描 {} 个文件）...",
                            scanned_files
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    pub(super) fn copy_directory_contents_with_progress(
        source_dir: &Path,
        target_dir: &Path,
        plan: &DirectoryCopyPlan,
        status: &mut DirectoryCopyStatus,
        progress: Option<&ProgressBar>,
    ) -> Result<()> {
        if !source_dir.exists() {
            anyhow::bail!(
                "Source directory for copy does not exist: {}",
                source_dir.display()
            );
        }
        std::fs::create_dir_all(target_dir).with_context(|| {
            format!(
                "Failed to create target directory for copy: {}",
                target_dir.display()
            )
        })?;

        for entry in std::fs::read_dir(source_dir)
            .with_context(|| format!("Failed to read source dir: {}", source_dir.display()))?
        {
            let entry = entry
                .with_context(|| format!("Failed to read entry in {}", source_dir.display()))?;
            let source_path = entry.path();
            let target_path = target_dir.join(entry.file_name());
            let metadata = std::fs::symlink_metadata(&source_path).with_context(|| {
                format!(
                    "Failed to read source metadata (without following symlink): {}",
                    source_path.display()
                )
            })?;
            if Self::is_symlink_or_reparse_point(&metadata) {
                anyhow::bail!(
                    "Refusing to import symbolic link/reparse point: {}",
                    source_path.display()
                );
            }

            if metadata.is_dir() {
                Self::copy_directory_contents_with_progress(
                    &source_path,
                    &target_path,
                    plan,
                    status,
                    progress,
                )?;
            } else if metadata.is_file() {
                Self::copy_file_with_progress(&source_path, &target_path, status, plan, progress)?;
            }
        }

        Ok(())
    }

    pub(super) fn copy_file_with_progress(
        source_path: &Path,
        target_path: &Path,
        status: &mut DirectoryCopyStatus,
        plan: &DirectoryCopyPlan,
        progress: Option<&ProgressBar>,
    ) -> Result<()> {
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent dir for {}", target_path.display())
            })?;
        }

        let source_file = std::fs::File::open(source_path).with_context(|| {
            format!(
                "Failed to open source file for copy: {}",
                source_path.display()
            )
        })?;
        let target_file = std::fs::File::create(target_path).with_context(|| {
            format!(
                "Failed to create target file for copy: {}",
                target_path.display()
            )
        })?;

        let mut reader = BufReader::new(source_file);
        let mut writer = BufWriter::new(target_file);
        let mut buffer = vec![0u8; 1024 * 1024];
        loop {
            let read = reader.read(&mut buffer).with_context(|| {
                format!(
                    "Failed to read source file chunk: {}",
                    source_path.display()
                )
            })?;
            if read == 0 {
                break;
            }
            writer.write_all(&buffer[..read]).with_context(|| {
                format!(
                    "Failed to write target file chunk: {}",
                    target_path.display()
                )
            })?;
            status.copied_bytes += read as u64;
            Self::update_adopt_progress(progress, plan, status);
        }
        writer
            .flush()
            .with_context(|| format!("Failed to flush target file: {}", target_path.display()))?;

        status.copied_files += 1;
        Self::update_adopt_progress(progress, plan, status);
        Ok(())
    }

    pub(super) fn update_adopt_progress(
        progress: Option<&ProgressBar>,
        plan: &DirectoryCopyPlan,
        status: &DirectoryCopyStatus,
    ) {
        let Some(pb) = progress else {
            return;
        };

        let stage_start = 35u64;
        let stage_end = 96u64;
        let stage_range = stage_end.saturating_sub(stage_start);

        let ratio = if plan.total_bytes > 0 {
            status.copied_bytes as f64 / plan.total_bytes as f64
        } else if plan.file_count > 0 {
            status.copied_files as f64 / plan.file_count as f64
        } else {
            1.0
        }
        .clamp(0.0, 1.0);

        let target_pos = stage_start + (ratio * stage_range as f64).round() as u64;
        if target_pos > pb.position() {
            pb.set_position(target_pos.min(stage_end));
        }

        pb.set_message(format!(
            "📂 正在导入系统 Python（{}/{} 文件）...",
            status.copied_files.min(plan.file_count),
            plan.file_count
        ));
    }

    pub(super) fn find_existing_system_python_dir(&self, version: &str) -> Result<Option<PathBuf>> {
        if !cfg!(windows) {
            return Ok(None);
        }

        let parsed = Version::parse(version)
            .with_context(|| format!("Invalid Python version: {version}"))?;
        let mut candidates = Self::build_default_python_dir_candidates(parsed.major, parsed.minor);
        candidates.extend(Self::collect_registry_python_dir_candidates(
            parsed.major,
            parsed.minor,
        ));
        if let Some(path) = Self::collect_py_launcher_python_dir(parsed.major, parsed.minor) {
            candidates.push(path);
        }
        let trusted_roots = Self::trusted_python_install_roots();

        let mut unique = HashSet::<String>::new();
        candidates.retain(|path| unique.insert(path.to_string_lossy().to_lowercase()));

        for candidate in candidates {
            let python_exe = candidate.join("python.exe");
            if !python_exe.exists() {
                continue;
            }

            if !Self::is_trusted_system_python_dir(&candidate, &trusted_roots) {
                warn!(
                    "Skip untrusted Python installation candidate outside trusted roots: {}",
                    candidate.display()
                );
                continue;
            }

            if Self::python_exe_matches_version(&python_exe, version)? {
                return Ok(Some(candidate));
            }
        }

        Ok(None)
    }

    pub(super) fn build_default_python_dir_candidates(major: u64, minor: u64) -> Vec<PathBuf> {
        let base_folder = format!("Python{}{}", major, minor);
        let folder_variants = [
            base_folder.clone(),
            format!("{base_folder}-64"),
            format!("{base_folder}-32"),
        ];
        let mut candidates = Vec::<PathBuf>::new();

        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let base = PathBuf::from(local_app_data)
                .join("Programs")
                .join("Python");
            for variant in &folder_variants {
                candidates.push(base.join(variant));
            }
        }
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            let base = PathBuf::from(program_files);
            for variant in &folder_variants {
                candidates.push(base.join(variant));
            }
        }
        if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
            let base = PathBuf::from(program_files_x86);
            for variant in &folder_variants {
                candidates.push(base.join(variant));
            }
        }

        candidates
    }

    pub(super) fn collect_registry_python_dir_candidates(major: u64, minor: u64) -> Vec<PathBuf> {
        let version_key = format!("{major}.{minor}");
        let registry_paths = [
            format!(r"HKCU\Software\Python\PythonCore\{version_key}\InstallPath"),
            format!(r"HKLM\Software\Python\PythonCore\{version_key}\InstallPath"),
            format!(r"HKLM\Software\WOW6432Node\Python\PythonCore\{version_key}\InstallPath"),
        ];

        let mut candidates = Vec::<PathBuf>::new();
        for registry_path in registry_paths {
            candidates.extend(Self::query_registry_install_paths(&registry_path));
        }
        candidates
    }

    pub(super) fn query_registry_install_paths(registry_path: &str) -> Vec<PathBuf> {
        let reg_exe = Self::windows_reg_exe();
        if !reg_exe.exists() {
            warn!("Registry command not found: {}", reg_exe.display());
            return Vec::new();
        }

        let output = Command::new(&reg_exe)
            .args(["query", registry_path])
            .output();
        let Ok(output) = output else {
            return Vec::new();
        };

        if !output.status.success() {
            return Vec::new();
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .lines()
            .filter_map(Self::extract_registry_install_path_from_line)
            .collect()
    }

    pub(super) fn extract_registry_install_path_from_line(line: &str) -> Option<PathBuf> {
        let (_, value_data) = line.split_once("REG_SZ")?;
        let raw = value_data.trim().trim_matches('"');
        if raw.is_empty() {
            return None;
        }

        let mut candidate = PathBuf::from(raw);
        if candidate
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("python.exe"))
        {
            candidate.pop();
        }

        if candidate.as_os_str().is_empty() {
            None
        } else {
            Some(candidate)
        }
    }

    pub(super) fn collect_py_launcher_python_dir(major: u64, minor: u64) -> Option<PathBuf> {
        let selector = format!("-{major}.{minor}");
        let py_launcher = Self::find_windows_py_launcher()?;
        let output = Command::new(&py_launcher)
            .args([selector.as_str(), "-c", "import sys; print(sys.executable)"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let first_line = stdout
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())?;
        let mut candidate = PathBuf::from(first_line.trim_matches('"'));

        if candidate
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("python.exe"))
        {
            candidate.pop();
        }

        if candidate.as_os_str().is_empty() {
            None
        } else {
            Some(candidate)
        }
    }

    pub(super) fn python_exe_matches_version(python_exe: &Path, version: &str) -> Result<bool> {
        let output = Command::new(python_exe)
            .arg("--version")
            .output()
            .with_context(|| format!("Failed to execute '{} --version'", python_exe.display()))?;

        if !output.status.success() {
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}\n{}", stdout.trim(), stderr.trim());
        Self::python_output_matches_requested_version(&combined, version)
    }
}
