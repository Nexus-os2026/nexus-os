# Lessons Learned

## Terminal Pasting
- Large heredoc blocks get corrupted when pasted into Linux terminal
- Use python scripts or nano editor for creating files with complex content
- Use simple dashes (-) not em dashes in git commit messages

## UI Issues
- Always make buttons actually do something - no dead buttons ever
- Agent IDs must be valid UUIDs, not string names
- When adding a dropdown selector, wire it all the way to the backend
- Loading indicators: only show ONE at a time
- Voice features need real Web Speech API, not just visual indicators
- Clear buttons must actually clear state
- Case-insensitive keyword matching for any text analysis
- Regex patterns for keyword detection must use /i flag and cover common variants (e.g., "Three Fiber" not just "react three fiber")
- Button state feedback: show transient text ("Starting...") for ~1s, disable when action is invalid for current state
- Always update "last action" display immediately on user interaction, don't wait for backend refresh

## Architecture
- Every agent action must go through kernel capability checks
- Fuel budget checked before execution, not after
- Audit trail is append-only - never modify events
- Mock data is okay for UI development but must be clearly labeled
