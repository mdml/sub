macro_rules! observing_methods {
    () => {
        /// Run one prompt turn and notify an observer as updates arrive.
        ///
        /// # Errors
        ///
        /// Returns [`AcpError`] when the agent process, negotiation, or turn fails.
        pub async fn prompt_turn_observing(
            &self,
            cwd: impl AsRef<Path>,
            prompt: &str,
            options: PromptOptions,
            observer: Option<UpdateObserver>,
        ) -> Result<(SessionHandle, PromptResult), AcpError> {
            self.prompt_turn_observing_session(cwd, prompt, options, observer, None)
                .await
        }

        /// Run one prompt turn and notify observers as the session opens and updates arrive.
        ///
        /// # Errors
        ///
        /// Returns [`AcpError`] when the agent process, negotiation, session open, or turn fails.
        pub async fn prompt_turn_observing_session(
            &self,
            cwd: impl AsRef<Path>,
            prompt: &str,
            options: PromptOptions,
            observer: Option<UpdateObserver>,
            session_observer: Option<SessionObserver>,
        ) -> Result<(SessionHandle, PromptResult), AcpError> {
            let timeout = options.timeout;
            let run = run_prompt_turn(
                self,
                PromptTurn {
                    cwd: cwd.as_ref().to_path_buf(),
                    prompt: prompt.to_owned(),
                    options,
                    observer,
                    session_observer,
                },
            );
            match timeout {
                Some(duration) => tokio::time::timeout(duration, run)
                    .await
                    .map_err(|_| AcpError::TimedOut(duration))?,
                None => run.await,
            }
        }
    };
}
