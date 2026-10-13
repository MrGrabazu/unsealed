import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { setCardLanguage } from "../api";
import { de } from "./de";
import { en, type MessageKey, type Messages } from "./en";
import { es } from "./es";
import { fr } from "./fr";
import { it } from "./it";

export type Lang = "en" | "fr" | "de" | "es" | "it";

/** Languages offered, each named in itself. */
export const LANGUAGES: { code: Lang; name: string }[] = [
  { code: "en", name: "English" },
  { code: "fr", name: "Français" },
  { code: "de", name: "Deutsch" },
  { code: "es", name: "Español" },
  { code: "it", name: "Italiano" },
];

const MESSAGES: Record<Lang, Messages> = { en, fr, de, es, it };
const STORAGE_KEY = "unsealed.language";

/** Languages cards and set names can be shown in (when printed in them; see the Rust
 * `FOREIGN_LANGUAGES`). Not all are interface languages. */
export type CardLang = Lang | "ja";
export const CARD_LANGUAGES: { code: CardLang; name: string }[] = [...LANGUAGES, { code: "ja", name: "日本語" }];
/** The card language setting: "auto" follows the interface language. */
export type CardLangSetting = CardLang | "auto";
const CARD_STORAGE_KEY = "unsealed.cardLanguage";

type Vars = Record<string, string | number>;
/** Keys that have plural forms (`<base>_one` and `<base>_other`). */
type PluralBase = { [K in MessageKey]: K extends `${infer Base}_one` ? Base : never }[MessageKey];

export interface I18n {
  lang: Lang;
  setLang: (lang: Lang) => void;
  cardLangSetting: CardLangSetting;
  setCardLangSetting: (setting: CardLangSetting) => void;
  /** The text of `key` in the current language, with its `{placeholders}` filled. */
  t: (key: MessageKey, vars?: Vars) => string;
  /** The plural form of `base` that fits `count` (also available as `{count}`). */
  plural: (base: PluralBase, count: number, vars?: Vars) => string;
  /** Unix time in seconds, as a medium date and short time. */
  formatDate: (seconds: number) => string;
  formatNumber: (n: number) => string;
}

const isLang = (value: string | null | undefined): value is Lang => LANGUAGES.some((l) => l.code === value);

/** The saved choice, else the system language when offered, else English. */
function initialLang(): Lang {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (isLang(saved)) return saved;
  const system = navigator.language?.slice(0, 2).toLowerCase();
  return isLang(system) ? system : "en";
}

const fill = (template: string, vars?: Vars) =>
  vars ? template.replace(/\{(\w+)\}/g, (match, name: string) => (name in vars ? String(vars[name]) : match)) : template;

const I18nContext = createContext<I18n | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(initialLang);

  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);

  const setLang = useCallback((next: Lang) => {
    localStorage.setItem(STORAGE_KEY, next);
    setLangState(next);
  }, []);

  const [cardLangSetting, setCardLangState] = useState<CardLangSetting>(() => {
    const saved = localStorage.getItem(CARD_STORAGE_KEY);
    return CARD_LANGUAGES.some((l) => l.code === saved) ? (saved as CardLang) : "auto";
  });
  const setCardLangSetting = useCallback((next: CardLangSetting) => {
    localStorage.setItem(CARD_STORAGE_KEY, next);
    setCardLangState(next);
  }, []);

  // Cards are named by the game itself: tell it the language to use.
  const cardLang = cardLangSetting === "auto" ? lang : cardLangSetting;
  useEffect(() => {
    setCardLanguage(cardLang).catch(() => {});
  }, [cardLang]);

  const value = useMemo<I18n>(() => {
    const messages = MESSAGES[lang];
    const rules = new Intl.PluralRules(lang);
    const dates = new Intl.DateTimeFormat(lang, { dateStyle: "medium", timeStyle: "short" });
    const numbers = new Intl.NumberFormat(lang);
    const t = (key: MessageKey, vars?: Vars) => fill(messages[key] ?? en[key], vars);
    return {
      lang,
      setLang,
      cardLangSetting,
      setCardLangSetting,
      t,
      plural: (base, count, vars) => {
        const form = rules.select(count) === "one" ? "one" : "other";
        return t(`${base}_${form}` as MessageKey, { count: numbers.format(count), ...vars });
      },
      formatDate: (seconds) => dates.format(new Date(seconds * 1000)),
      formatNumber: (n) => numbers.format(n),
    };
  }, [lang, setLang, cardLangSetting, setCardLangSetting]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18n {
  const i18n = useContext(I18nContext);
  if (!i18n) throw new Error("useI18n needs an I18nProvider");
  return i18n;
}
