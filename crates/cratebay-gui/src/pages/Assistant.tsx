import { useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { I } from "../icons"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import {
  defaultSkillInputValue,
  formatSkillExecutionOutput,
  skillNeedsConfirmation,
  skillUsesPromptInput,
} from "@/lib/aiSkills"
import type {
  AiSettings,
  AiSkillDefinition,
  AiSkillExecutionResult,
  AssistantPlanResult,
  AssistantPlanStep,
  AssistantStepExecutionResult,
} from "../types"

interface AssistantProps {
  t: (key: string) => string
}

const enabledSkillsFromSettings = (settings: AiSettings | null) =>
  Array.isArray(settings?.skills) ? settings.skills.filter((skill) => skill.enabled) : []

export function Assistant({ t }: AssistantProps) {
  const [prompt, setPrompt] = useState("")
  const [plan, setPlan] = useState<AssistantPlanResult | null>(null)
  const [planError, setPlanError] = useState("")
  const [generating, setGenerating] = useState(false)
  const [executingStepId, setExecutingStepId] = useState("")
  const [stepArgsMap, setStepArgsMap] = useState<Record<string, string>>({})
  const [stepResultMap, setStepResultMap] = useState<Record<string, string>>({})
  const [skillsLoading, setSkillsLoading] = useState(true)
  const [skillsError, setSkillsError] = useState("")
  const [skills, setSkills] = useState<AiSkillDefinition[]>([])
  const [skillInputMap, setSkillInputMap] = useState<Record<string, string>>({})
  const [skillDryRunMap, setSkillDryRunMap] = useState<Record<string, boolean>>({})
  const [skillResultMap, setSkillResultMap] = useState<Record<string, string>>({})
  const [skillErrorMap, setSkillErrorMap] = useState<Record<string, string>>({})
  const [runningSkillId, setRunningSkillId] = useState("")

  const canGenerate = useMemo(() => prompt.trim().length > 0 && !generating, [prompt, generating])

  useEffect(() => {
    let active = true

    const loadSkills = async () => {
      setSkillsLoading(true)
      setSkillsError("")
      try {
        const settings = await invoke<AiSettings | null>("load_ai_settings")
        if (!active) return

        const nextSkills = enabledSkillsFromSettings(settings)
        setSkills(nextSkills)
        setSkillInputMap((prev) => {
          const next = { ...prev }
          for (const skill of nextSkills) {
            if (!(skill.id in next)) {
              next[skill.id] = defaultSkillInputValue(skill)
            }
          }
          return next
        })
        setSkillDryRunMap((prev) => {
          const next = { ...prev }
          for (const skill of nextSkills) {
            if (!(skill.id in next)) {
              next[skill.id] = skill.executor === "agent_cli_preset"
            }
          }
          return next
        })
      } catch (e) {
        if (!active) return
        setSkills([])
        setSkillsError(String(e))
      } finally {
        if (active) {
          setSkillsLoading(false)
        }
      }
    }

    loadSkills()
    return () => {
      active = false
    }
  }, [])

  const handleGeneratePlan = async () => {
    if (!prompt.trim()) return
    setGenerating(true)
    setPlanError("")
    setPlan(null)
    setStepResultMap({})
    try {
      const result = await invoke<AssistantPlanResult>("ai_generate_plan", {
        prompt: prompt.trim(),
        preferModel: true,
      })
      setPlan(result)
      const nextArgs: Record<string, string> = {}
      for (const step of result.steps) {
        nextArgs[step.id] = JSON.stringify(step.args ?? {}, null, 2)
      }
      setStepArgsMap(nextArgs)
    } catch (e) {
      setPlanError(String(e))
    } finally {
      setGenerating(false)
    }
  }

  const runStep = async (step: AssistantPlanStep) => {
    const rawArgs = stepArgsMap[step.id] ?? "{}"
    let argsObj: Record<string, unknown> = {}
    try {
      const parsed = JSON.parse(rawArgs) as unknown
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        throw new Error("args must be an object")
      }
      argsObj = parsed as Record<string, unknown>
    } catch (e) {
      setStepResultMap((prev) => ({
        ...prev,
        [step.id]: `Invalid args JSON: ${String(e)}`,
      }))
      return
    }

    const confirmed = step.requires_confirmation
      ? window.confirm(`${t("assistantConfirmAction")}\n${step.title}\n${step.command}`)
      : true
    if (!confirmed) return

    setExecutingStepId(step.id)
    setStepResultMap((prev) => ({ ...prev, [step.id]: t("working") }))
    try {
      const result = await invoke<AssistantStepExecutionResult>("assistant_execute_step", {
        command: step.command,
        args: argsObj,
        riskLevel: step.risk_level,
        requiresConfirmation: step.requires_confirmation,
        confirmed,
      })
      const output =
        typeof result.output === "string"
          ? result.output
          : JSON.stringify(result.output, null, 2) || t("done")
      setStepResultMap((prev) => ({
        ...prev,
        [step.id]: result.request_id ? `${output}\nrequest_id=${result.request_id}` : output,
      }))
    } catch (e) {
      setStepResultMap((prev) => ({
        ...prev,
        [step.id]: String(e),
      }))
    } finally {
      setExecutingStepId("")
    }
  }

  const runSkill = async (skill: AiSkillDefinition) => {
    setRunningSkillId(skill.id)
    setSkillErrorMap((prev) => ({ ...prev, [skill.id]: "" }))
    setSkillResultMap((prev) => ({ ...prev, [skill.id]: "" }))

    try {
      let input: unknown
      if (skillUsesPromptInput(skill)) {
        const promptValue = (skillInputMap[skill.id] ?? "").trim()
        if (!promptValue) {
          throw new Error(`${t("agentCliPrompt")} is required`)
        }
        input = { prompt: promptValue }
      } else {
        const rawInput = skillInputMap[skill.id] ?? "{}"
        const parsed = JSON.parse(rawInput) as unknown
        if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
          throw new Error("skill input must be a JSON object")
        }
        input = parsed
      }

      const needsConfirmation = skillNeedsConfirmation(skill)
      const confirmed = needsConfirmation
        ? window.confirm(`${t("assistantConfirmAction")}\n${skill.display_name}\n${skill.target}`)
        : null
      if (needsConfirmation && !confirmed) {
        return
      }

      const result = await invoke<AiSkillExecutionResult>("ai_skill_execute", {
        skillId: skill.id,
        input,
        dryRun: skill.executor === "agent_cli_preset" ? (skillDryRunMap[skill.id] ?? true) : null,
        confirmed,
      })

      setSkillResultMap((prev) => ({
        ...prev,
        [skill.id]: formatSkillExecutionOutput(result, t("done")),
      }))
    } catch (e) {
      setSkillErrorMap((prev) => ({
        ...prev,
        [skill.id]: String(e),
      }))
    } finally {
      setRunningSkillId("")
    }
  }

  return (
    <div className="space-y-4">
      <Card className="py-0">
        <CardContent className="space-y-3 py-4">
          <div className="flex items-center gap-3">
            <div className="size-10 shrink-0 rounded-lg bg-primary/10 text-primary flex items-center justify-center [&_svg]:size-5 [&_svg]:fill-none [&_svg]:stroke-current [&_svg]:stroke-2 [&_svg]:stroke-linecap-round [&_svg]:stroke-linejoin-round">
              {I.aiAssistant}
            </div>
            <div className="min-w-0 flex-1">
              <div className="text-sm font-semibold text-foreground">{t("assistant")}</div>
              <div className="text-xs text-muted-foreground">{t("assistantDesc")}</div>
            </div>
          </div>

          <textarea
            value={prompt}
            onChange={(event) => setPrompt(event.target.value)}
            spellCheck={false}
            className="min-h-24 w-full rounded-md border border-border bg-background px-3 py-2 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            placeholder={t("assistantPromptPlaceholder")}
          />

          <div className="flex items-center gap-2">
            <Button
              type="button"
              variant="secondary"
              size="sm"
              onClick={handleGeneratePlan}
              disabled={!canGenerate}
            >
              {generating ? t("working") : t("assistantGeneratePlan")}
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card className="py-0">
        <CardContent className="space-y-3 py-4">
          <div>
            <div className="text-sm font-semibold text-foreground">{t("assistantQuickSkills")}</div>
            <div className="text-xs text-muted-foreground">{t("assistantQuickSkillsDesc")}</div>
          </div>

          {skillsLoading && <div className="text-xs text-muted-foreground">{t("working")}</div>}

          {skillsError && (
            <Alert variant="destructive">
              <AlertTitle>{t("assistantQuickSkills")}</AlertTitle>
              <AlertDescription>
                <p className="whitespace-pre-wrap">{skillsError}</p>
              </AlertDescription>
            </Alert>
          )}

          {!skillsLoading && !skillsError && skills.length === 0 && (
            <div className="text-xs text-muted-foreground">{t("aiSkillsEmpty")}</div>
          )}

          {skills.map((skill) => (
            <div
              key={skill.id}
              data-testid={`assistant-skill-card-${skill.id}`}
              className="space-y-2 rounded-md border border-border/70 bg-card px-3 py-3"
            >
              <div className="flex flex-wrap items-center gap-2">
                <div className="text-sm font-semibold text-foreground">{skill.display_name}</div>
                <span className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
                  {skill.executor}
                </span>
                <span className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
                  {skill.target}
                </span>
              </div>
              <div className="text-xs text-muted-foreground">{skill.description}</div>

              <div>
                <label className="text-xs font-semibold text-muted-foreground">
                  {skillUsesPromptInput(skill) ? t("agentCliPrompt") : t("assistantStepArgs")}
                </label>
                <textarea
                  data-testid={`assistant-skill-input-${skill.id}`}
                  value={skillInputMap[skill.id] ?? defaultSkillInputValue(skill)}
                  onChange={(event) =>
                    setSkillInputMap((prev) => ({
                      ...prev,
                      [skill.id]: event.target.value,
                    }))
                  }
                  spellCheck={false}
                  className="mt-1 min-h-24 w-full rounded-md border border-border bg-background px-3 py-2 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                />
              </div>

              {skill.executor === "agent_cli_preset" && (
                <label className="flex items-center gap-2 text-sm text-muted-foreground">
                  <Checkbox
                    checked={skillDryRunMap[skill.id] ?? true}
                    onCheckedChange={(value) =>
                      setSkillDryRunMap((prev) => ({
                        ...prev,
                        [skill.id]: value === true,
                      }))
                    }
                  />
                  <span>{t("agentCliDryRun")}</span>
                </label>
              )}

              <div className="flex items-center gap-2">
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => runSkill(skill)}
                  disabled={runningSkillId === skill.id}
                >
                  {runningSkillId === skill.id ? t("working") : t("aiSkillRun")}
                </Button>
              </div>

              {skillResultMap[skill.id] && (
                <pre
                  data-testid={`assistant-skill-result-${skill.id}`}
                  className="max-h-40 overflow-auto whitespace-pre-wrap rounded-md bg-background px-2 py-2 text-xs text-muted-foreground"
                >
                  {skillResultMap[skill.id]}
                </pre>
              )}

              {skillErrorMap[skill.id] && (
                <Alert variant="destructive">
                  <AlertTitle>{skill.display_name}</AlertTitle>
                  <AlertDescription>
                    <p className="whitespace-pre-wrap">{skillErrorMap[skill.id]}</p>
                  </AlertDescription>
                </Alert>
              )}
            </div>
          ))}
        </CardContent>
      </Card>

      {planError && (
        <Alert variant="destructive">
          <AlertTitle>{t("assistant")}</AlertTitle>
          <AlertDescription>
            <p className="whitespace-pre-wrap">{planError}</p>
          </AlertDescription>
        </Alert>
      )}

      {plan && (
        <Card className="py-0">
          <CardContent className="space-y-4 py-4">
            <div className="text-sm text-muted-foreground">
              <strong>{t("assistantStrategy")}:</strong> {plan.strategy}
              {plan.fallback_used ? ` · ${t("assistantFallbackUsed")}` : ""}
              {plan.request_id ? ` · request_id=${plan.request_id}` : ""}
            </div>
            <div className="text-sm text-foreground">{plan.notes}</div>

            <div className="space-y-3">
              {plan.steps.map((step) => (
                <div
                  key={step.id}
                  className="rounded-md border border-border/70 bg-card px-3 py-3"
                >
                  <div className="flex flex-wrap items-center gap-2">
                    <div className="text-sm font-semibold text-foreground">{step.title}</div>
                    <span className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
                      {step.command}
                    </span>
                    <span className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
                      {step.risk_level}
                    </span>
                    {step.requires_confirmation && (
                      <span className="rounded bg-amber-100 px-1.5 py-0.5 text-[10px] text-amber-700 dark:bg-amber-950 dark:text-amber-300">
                        {t("assistantNeedConfirm")}
                      </span>
                    )}
                  </div>
                  <div className="mt-1 text-xs text-muted-foreground">{step.explain}</div>

                  <div className="mt-2">
                    <label className="text-xs font-semibold text-muted-foreground">
                      {t("assistantStepArgs")}
                    </label>
                    <textarea
                      data-testid={`assistant-step-args-${step.id}`}
                      value={stepArgsMap[step.id] ?? "{}"}
                      onChange={(event) =>
                        setStepArgsMap((prev) => ({
                          ...prev,
                          [step.id]: event.target.value,
                        }))
                      }
                      spellCheck={false}
                      className="mt-1 min-h-24 w-full rounded-md border border-border bg-background px-3 py-2 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                    />
                  </div>

                  <div className="mt-2 flex items-center gap-2">
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => runStep(step)}
                      disabled={executingStepId === step.id}
                    >
                      {executingStepId === step.id ? t("working") : t("assistantRunStep")}
                    </Button>
                  </div>

                  {stepResultMap[step.id] && (
                    <pre
                      data-testid={`assistant-step-result-${step.id}`}
                      className="mt-2 max-h-40 overflow-auto whitespace-pre-wrap rounded-md bg-background px-2 py-2 text-xs text-muted-foreground"
                    >
                      {stepResultMap[step.id]}
                    </pre>
                  )}
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
