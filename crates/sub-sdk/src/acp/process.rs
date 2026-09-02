use std::io;
use std::time::Duration;

use agent_client_protocol::{AcpAgent, ByteStreams};
use async_process::{Child, ChildStdin, ChildStdout};

use super::AcpError;

const SHUTDOWN_GRACE_PERIOD: Duration = Duration::from_secs(1);

pub(super) struct AgentProcess {
    child: Option<Child>,
    stderr_task: tokio::task::JoinHandle<()>,
}

impl AgentProcess {
    pub(super) fn spawn(
        agent: &AcpAgent,
    ) -> Result<(Self, ByteStreams<ChildStdin, ChildStdout>), AcpError> {
        let (stdin, stdout, mut stderr, child) = agent
            .spawn_process()
            .map_err(|error| AcpError::Protocol(error.to_string()))?;
        let stderr_task = tokio::spawn(async move {
            let _ = futures::io::copy(&mut stderr, &mut futures::io::sink()).await;
        });
        Ok((
            Self {
                child: Some(child),
                stderr_task,
            },
            ByteStreams::new(stdin, stdout),
        ))
    }

    pub(super) async fn shutdown(&mut self, force: bool) -> Result<(), AcpError> {
        if force {
            self.terminate_group()?;
            self.wait().await?;
        } else if let Ok(status) = tokio::time::timeout(SHUTDOWN_GRACE_PERIOD, self.wait()).await {
            if !status?.success() {
                return Err(AcpError::Protocol(
                    "agent process exited unsuccessfully".to_owned(),
                ));
            }
        } else {
            self.terminate_group()?;
            self.wait().await?;
        }
        self.stderr_task.abort();
        Ok(())
    }

    async fn wait(&mut self) -> Result<std::process::ExitStatus, AcpError> {
        let Some(child) = self.child.as_mut() else {
            return Err(AcpError::ProcessExited);
        };
        let status = child.status().await.map_err(AcpError::Io)?;
        self.child = None;
        Ok(status)
    }

    #[allow(unsafe_code)]
    fn terminate_group(&mut self) -> io::Result<()> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        let pid = i32::try_from(child.id())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "child PID exceeds i32"))?;
        let result = unsafe { libc::kill(-pid, libc::SIGKILL) };
        if result == -1 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
        if let Err(error) = child.kill()
            && error.raw_os_error() != Some(libc::ESRCH)
        {
            return Err(error);
        }
        Ok(())
    }
}

impl Drop for AgentProcess {
    fn drop(&mut self) {
        let _ = self.terminate_group();
        self.stderr_task.abort();
    }
}
