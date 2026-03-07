import type { AiSkillDefinition, AiSkillExecutionResult } from "../types"

export const skillUsesPromptInput = (skill: AiSkillDefinition) =>
  skill.executor === "agent_cli_preset"

export const defaultSkillInputValue = (skill: AiSkillDefinition) => {
  if (skillUsesPromptInput(skill)) return ""

  switch (skill.target) {
    case "k8s_list_pods":
      return JSON.stringify({ namespace: "default" }, null, 2)
    case "sandbox_exec":
      return JSON.stringify(
        { id: "<sandbox-id>", command: "echo hello from sandbox" },
        null,
        2
      )
    default:
      return "{}"
  }
}

export const skillNeedsConfirmation = (skill: AiSkillDefinition) =>
  !skillUsesPromptInput(skill) &&
  /(delete|remove|destroy|drop|wipe|prune|terminate|kill|uninstall|purge|stop|start|exec|run|create|install|update)/i.test(
    skill.target
  )

export const formatSkillExecutionOutput = (
  result: Pick<AiSkillExecutionResult, "output" | "request_id">,
  doneLabel: string
) => {
  const output =
    typeof result.output === "string"
      ? result.output
      : JSON.stringify(result.output, null, 2) || doneLabel

  return result.request_id ? `${output}\nrequest_id=${result.request_id}` : output
}
