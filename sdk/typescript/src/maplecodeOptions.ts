export type MaplecodeConfigValue = string | number | boolean | MaplecodeConfigValue[] | MaplecodeConfigObject;

export type MaplecodeConfigObject = { [key: string]: MaplecodeConfigValue };

export type MaplecodeOptions = {
  maplecodePathOverride?: string;
  baseUrl?: string;
  apiKey?: string;
  /**
   * Additional `--config key=value` overrides to pass to the Maplecode CLI.
   *
   * Provide a JSON object and the SDK will flatten it into dotted paths and
   * serialize values as TOML literals so they are compatible with the CLI's
   * `--config` parsing.
   */
  config?: MaplecodeConfigObject;
  /**
   * Raw `--config key=value` overrides to pass unchanged to the Maplecode CLI after
   * structured configuration and before SDK-managed or thread-specific overrides.
   */
  configOverrides?: string[];
  /**
   * Environment variables passed to the Maplecode CLI process. When provided, the SDK
   * will not inherit variables from `process.env`.
   */
  env?: Record<string, string>;
};
