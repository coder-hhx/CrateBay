import { useEffect, useRef } from "react";
import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/i18n";
import { User, Bot } from "lucide-react";
import { Streamdown } from "streamdown";
import type { ChatMessage } from "@/types/chat";
import { ThinkingBlock } from "./ThinkingBlock";
import { ToolCallItem } from "./ToolCallItem";

interface MessageBubbleProps {
  message: ChatMessage;
  isThinking?: boolean;
  thinkingContent?: string;
}

/**
 * Single message bubble with Streamdown rendering for assistant messages.
 * Includes ThinkingBlock and ToolCallItem components.
 *
 * Visual design:
 * - User messages: right-aligned, purple translucent bg, rounded-2xl with tr-sm
 * - AI messages: left-aligned, card bg with border
 * - Avatars: gradient circles (purple for user, cyan for AI)
 * - Fade-in animation on mount
 */
export function MessageBubble({ message, isThinking, thinkingContent }: MessageBubbleProps) {
  const { t } = useI18n();
  const isUser = message.role === "user";
  const isAssistant = message.role === "assistant";
  const isStreaming = message.status === "streaming";
  const streamdownRef = useRef<HTMLDivElement>(null);

  // Apply streaming-friendly class to Streamdown container
  useEffect(() => {
    if (streamdownRef.current !== null && isStreaming) {
      streamdownRef.current.scrollIntoView({ behavior: "smooth", block: "end" });
    }
  }, [message.content, isStreaming]);

  return (
    <div
      className={cn(
        "flex gap-3 animate-in fade-in slide-in-from-bottom-2 duration-250",
        isUser ? "flex-row-reverse" : "flex-row",
      )}
    >
      {/* Avatar */}
      <div
        className={cn(
          "flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full",
          isUser
            ? "bg-gradient-to-br from-[#7c3aed] to-[#6d28d9] text-white"
            : "bg-gradient-to-br from-[#22d3ee] to-[#06b6d4] text-white",
        )}
      >
        {isUser ? <User className="h-4 w-4" /> : <Bot className="h-4 w-4" />}
      </div>

      {/* Message content */}
      <div
        className={cn(
          "max-w-[85%] min-w-0 text-sm leading-relaxed",
          isUser
            ? "rounded-2xl rounded-tr-sm border border-primary/20 bg-primary/10 px-4 py-3 text-foreground"
            : "",
        )}
      >
        {/* User message: simple text */}
        {isUser && (
          <div className="whitespace-pre-wrap break-words">
            {message.content}
          </div>
        )}

        {/* Assistant message: structured content */}
        {!isUser && (
          <div>
            {/* Thinking block — replaces AgentThinking */}
            {isAssistant && message.reasoning !== undefined && (
              <ThinkingBlock
                content={message.reasoning}
                isActive={isThinking ?? false}
              />
            )}
            {isAssistant && isThinking === true && thinkingContent !== undefined && message.reasoning === undefined && (
              <ThinkingBlock
                content={thinkingContent}
                isActive
              />
            )}

            {/* Tool call items — replaces ToolCallCard */}
            {message.toolCalls !== undefined && message.toolCalls.length > 0 && (
              <div className="my-1 space-y-1.5">
                {message.toolCalls.map((tc) => (
                  <ToolCallItem key={tc.id} toolCall={tc} />
                ))}
              </div>
            )}

            {/* Content — use Streamdown for assistant */}
            <div ref={streamdownRef} className="leading-relaxed text-foreground">
              <Streamdown mode={isStreaming ? "streaming" : "static"}>
                {message.content}
              </Streamdown>
            </div>
          </div>
        )}

        {/* Error indicator */}
        {message.status === "error" && (
          <p className="mt-1.5 text-xs text-destructive">
            {t("chat", "failedToSend")}
          </p>
        )}
      </div>
    </div>
  );
}
