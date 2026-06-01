/**
 * Typesafe i18n system for CrateBay.
 *
 * Matches frontend-spec.md §9 — i18n Strategy.
 * TypeScript validates both namespace and key at compile time.
 */
import { useMemo } from "react";
import type { Translations } from "@/types/i18n";
import en from "@/locales/en";
import zhCN from "@/locales/zh-CN";
import { useSettingsStore } from "@/stores/settingsStore";

const locales: Record<string, Translations> = {
  en,
  "zh-CN": zhCN,
};

interface I18nInstance {
  t: I18nFunction;
  locale: string;
}

interface I18nFunction {
  <K extends keyof Translations>(namespace: K): Translations[K];
  <K extends keyof Translations, S extends keyof Translations[K]>(
    namespace: K,
    key: S,
  ): string;
}

/**
 * Create an i18n instance for the given locale.
 */
export function createI18n(locale: string): I18nInstance {
  const translations = locales[locale] ?? locales.en;

  function t<K extends keyof Translations>(namespace: K): Translations[K];
  function t<K extends keyof Translations, S extends keyof Translations[K]>(
    namespace: K,
    key: S,
  ): string;
  function t(namespace: string, key?: string): unknown {
    const ns = (translations as unknown as Record<string, Record<string, string>>)[namespace];
    if (ns === undefined) return key ?? namespace;
    if (key !== undefined) return ns[key] ?? key;
    return ns;
  }

  return { t: t as I18nFunction, locale };
}

/**
 * React hook for i18n. Returns a typesafe `t` function and current locale.
 * Reads the language setting from settingsStore.
 *
 * @example
 * const { t } = useI18n();
 * t("images", "title"); // TypeScript validates both namespace and key
 */
export function useI18n(): I18nInstance {
  const language = useSettingsStore((s) => s.settings.language);
  return useMemo(() => createI18n(language), [language]);
}
