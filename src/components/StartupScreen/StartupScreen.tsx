import styles from "../WelcomeScreen/WelcomeScreen.module.css";

interface StartupScreenProps {
  error?: string;
}

/** Shown while `ResolveStartupPathUseCase` is resolving PATH, or if it failed. */
export function StartupScreen({ error }: StartupScreenProps) {
  return (
    <div className={styles.container}>
      <div className={styles.title}>{error ? "Failed to resolve environment" : "Resolving environment…"}</div>
      {error && <div>{error}</div>}
    </div>
  );
}
