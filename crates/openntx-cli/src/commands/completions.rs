use openntx_core::Result;

pub fn run(shell: &str) -> Result<()> {
    match shell {
        "bash" => {
            println!("# OpenNTX bash completions");
            println!("# Add to ~/.bashrc: source <(openntx completions bash)");
            println!();
            println!("complete -F _openntx_completions openntx");
            println!();
            println!("_openntx_completions() {{");
            println!("    local cur prev commands");
            println!("    COMPREPLY=()");
            println!("    cur=\"${{COMP_WORDS[COMP_CWORD]}}\"");
            println!("    prev=\"${{COMP_WORDS[COMP_CWORD-1]}}\"");
            println!("    commands=\"analyze run install desktop manifest capture package remove list show doctor rename duplicate export import logs config completions\"");
            println!();
            println!("    if [[ ${{COMP_CWORD}} -eq 1 ]]; then");
            println!("        COMPREPLY=( $(compgen -W \"$commands\" -- \"$cur\") )");
            println!("        return 0");
            println!("    fi");
            println!("}}");
        }
        "zsh" => {
            println!("# OpenNTX zsh completions");
            println!("# Add to ~/.zshrc: source <(openntx completions zsh)");
            println!();
            println!("#compdef openntx");
            println!();
            println!("_openntx() {{");
            println!("    local -a commands");
            println!("    commands=(");
            println!("        'analyze:Analyze a PE/EXE file'");
            println!("        'run:Create a run plan for a registered app'");
            println!("        'install:Analyze and register a PE/EXE'");
            println!("        'desktop:Manage desktop launchers'");
            println!("        'manifest:Generate manifests'");
            println!("        'capture:Capture snapshot/diff/report tools'");
            println!("        'package:Build .deb packages'");
            println!("        'remove:Remove a registered app'");
            println!("        'list:List registered apps'");
            println!("        'show:Show app details'");
            println!("        'doctor:Run diagnostics'");
            println!("        'rename:Rename an app'");
            println!("        'duplicate:Duplicate an app'");
            println!("        'export:Export app as bundle'");
            println!("        'import:Import an OpenNTX bundle'");
            println!("        'logs:Manage run-plan logs'");
            println!("        'config:Manage configuration'");
            println!("        'completions:Generate shell completions'");
            println!("    )");
            println!("    _describe 'command' commands");
            println!("}}");
            println!();
            println!("_openntx \"$@\"");
        }
        "fish" => {
            println!("# OpenNTX fish completions");
            println!("# Save to ~/.config/fish/completions/openntx.fish");
            println!();
            println!("complete -c openntx -f");
            println!("complete -c openntx -n '__fish_use_subcommand' -a analyze -d 'Analyze a PE/EXE file'");
            println!(
                "complete -c openntx -n '__fish_use_subcommand' -a run -d 'Create a run plan'"
            );
            println!("complete -c openntx -n '__fish_use_subcommand' -a install -d 'Analyze and register a PE/EXE'");
            println!("complete -c openntx -n '__fish_use_subcommand' -a desktop -d 'Manage desktop launchers'");
            println!("complete -c openntx -n '__fish_use_subcommand' -a manifest -d 'Generate manifests'");
            println!(
                "complete -c openntx -n '__fish_use_subcommand' -a capture -d 'Capture tools'"
            );
            println!("complete -c openntx -n '__fish_use_subcommand' -a package -d 'Build .deb packages'");
            println!("complete -c openntx -n '__fish_use_subcommand' -a remove -d 'Remove a registered app'");
            println!(
                "complete -c openntx -n '__fish_use_subcommand' -a list -d 'List registered apps'"
            );
            println!(
                "complete -c openntx -n '__fish_use_subcommand' -a show -d 'Show app details'"
            );
            println!(
                "complete -c openntx -n '__fish_use_subcommand' -a doctor -d 'Run diagnostics'"
            );
            println!("complete -c openntx -n '__fish_use_subcommand' -a rename -d 'Rename an app'");
            println!(
                "complete -c openntx -n '__fish_use_subcommand' -a duplicate -d 'Duplicate an app'"
            );
            println!("complete -c openntx -n '__fish_use_subcommand' -a export -d 'Export app as bundle'");
            println!("complete -c openntx -n '__fish_use_subcommand' -a import -d 'Import an OpenNTX bundle'");
            println!(
                "complete -c openntx -n '__fish_use_subcommand' -a logs -d 'Manage run-plan logs'"
            );
            println!("complete -c openntx -n '__fish_use_subcommand' -a config -d 'Manage configuration'");
            println!("complete -c openntx -n '__fish_use_subcommand' -a completions -d 'Generate shell completions'");
        }
        _ => {
            return Err(openntx_core::OpenNtxError::InvalidInput(format!(
                "unsupported shell: {shell}. Supported: bash, zsh, fish"
            )));
        }
    }

    Ok(())
}
