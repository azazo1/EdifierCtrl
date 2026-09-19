package dev.edifierctrl.app.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp

internal class ReadingDraft(initial: String, edited: Boolean = false) {
    var text by mutableStateOf(initial)
        private set
    var edited by mutableStateOf(edited)
        private set
    var focused by mutableStateOf(false)

    fun edit(value: String) {
        text = value
        edited = true
    }

    fun reset(confirmed: String) {
        text = confirmed
        edited = false
    }

    fun synchronize(confirmed: String) {
        if (!focused && (!edited || text == confirmed)) reset(confirmed)
    }
}

@Composable
internal fun rememberReadingDraft(scope: String, field: String, confirmed: String): ReadingDraft {
    val draft = rememberSaveable(
        scope,
        field,
        saver = listSaver<ReadingDraft, Any>(
            save = { listOf(it.text, it.edited) },
            restore = { ReadingDraft(it[0] as String, it[1] as Boolean) },
        ),
    ) { ReadingDraft(confirmed) }
    LaunchedEffect(draft, confirmed, draft.focused) {
        draft.synchronize(confirmed)
    }
    return draft
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun DraftActions(
    changed: Boolean,
    enabled: Boolean,
    valid: Boolean = true,
    onReset: () -> Unit,
    onApply: () -> Unit,
    applyLabel: String = "应用草稿",
) {
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        FilledTonalButton(
            onClick = onApply,
            enabled = enabled && changed && valid,
            modifier = Modifier.heightIn(min = 48.dp),
        ) { Text(applyLabel) }
        TextButton(
            onClick = onReset,
            enabled = changed,
            modifier = Modifier.heightIn(min = 48.dp),
        ) { Text("还原为当前读数") }
    }
}

@Composable
internal fun NumberDraftField(
    title: String,
    confirmed: Int?,
    range: IntRange,
    scope: String,
    field: String,
    enabled: Boolean,
    unit: String = "",
    detail: String? = null,
    allowUnchanged: Boolean = false,
    onApply: (Int) -> Unit,
) {
    val reading = confirmed?.toString().orEmpty()
    val draft = rememberReadingDraft(scope, field, reading)
    val value = draft.text.toIntOrNull()
    val valid = value != null && value in range
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(title, style = MaterialTheme.typography.titleSmall)
        Text(
            confirmed?.let { "当前确认: $it$unit" } ?: "等待耳机回报",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (detail != null) {
            Text(detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        OutlinedTextField(
            value = draft.text,
            onValueChange = draft::edit,
            enabled = enabled,
            label = { Text(if (unit.isEmpty()) "待应用值" else "待应用值 ($unit)") },
            placeholder = { Text("${range.first} 到 ${range.last}") },
            supportingText = { Text("范围 ${range.first} 到 ${range.last}$unit, 应用后等待读数确认.") },
            isError = draft.text.isNotEmpty() && !valid,
            singleLine = true,
            keyboardOptions = KeyboardOptions(
                keyboardType = if (range.first < 0) KeyboardType.Text else KeyboardType.Number,
            ),
            modifier = Modifier.fillMaxWidth().onFocusChanged { draft.focused = it.isFocused },
        )
        DraftActions(
            changed = draft.text != reading || allowUnchanged,
            enabled = enabled,
            valid = valid,
            onReset = { draft.reset(reading) },
            onApply = { if (value != null && value in range) onApply(value) },
        )
    }
}
