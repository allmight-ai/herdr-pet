//! Perguntas sobre processos do sistema. Hoje só uma: esse pid ainda existe?

use std::path::Path;

/// O processo `pid` ainda roda? `None` quando não dá pra saber — sem `/proc` e
/// sem `kill` utilizável. A dúvida é do chamador: a varredura de tmp poupa o
/// arquivo (apagar tmp vivo custa um save), a detecção de subagente descarta a
/// sessão (filho fantasma inflaria o `⚙ N` e o XP).
pub fn pid_alive(pid: u32) -> Option<bool> {
    if pid == 0 {
        return Some(false); // pid 0 não é processo de usuário
    }
    if Path::new("/proc/self").exists() {
        return Some(Path::new(&format!("/proc/{pid}")).exists());
    }
    // macOS e afins: `kill -0` não envia sinal, só testa a existência.
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .ok()
        .map(|o| o.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_proprio_processo_esta_vivo() {
        assert_eq!(pid_alive(std::process::id()), Some(true));
    }

    #[test]
    fn pid_zero_nunca_vive() {
        assert_eq!(pid_alive(0), Some(false));
    }

    #[test]
    fn pid_impossivel_nao_vive() {
        // u32::MAX está acima de qualquer pid_max real.
        assert_eq!(pid_alive(u32::MAX), Some(false));
    }
}
