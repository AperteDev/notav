package dev.aperte.notav.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.DropdownMenuGroup
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.DropdownMenuPopup
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.MenuDefaults
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.ToggleButtonDefaults
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TooltipAnchorPosition
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberTooltipState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp

// Todav's ui/Common.kt, the parts Notav uses.

/**
 * The widest content gets on a wide screen, centred: pages of settings and steps. Inside,
 * [Group]s keep 16dp from its edges.
 */
val CONTENT_WIDTH = 640.dp

/** Rows as one block: each a box of its own, the block's round corners at its ends. */
@Composable
fun Group(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) = Column(
    modifier.clip(RoundedCornerShape(24.dp)),
    verticalArrangement = Arrangement.spacedBy(2.dp),
    content = content,
)

private val ITEM_SHAPE = RoundedCornerShape(4.dp)

/** A row of a [Group]. */
@Composable
fun Item(
    headlineContent: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    supportingContent: @Composable (() -> Unit)? = null,
    leadingContent: @Composable (() -> Unit)? = null,
    trailingContent: @Composable (() -> Unit)? = null,
    containerColor: Color = MaterialTheme.colorScheme.surfaceContainer,
) {
    ListItem(
        modifier = Modifier.clip(ITEM_SHAPE).then(modifier),
        leadingContent = leadingContent,
        trailingContent = trailingContent,
        supportingContent = supportingContent,
        shapes = ListItemDefaults.shapes(shape = ITEM_SHAPE),
        colors = ListItemDefaults.segmentedColors(containerColor = containerColor),
        content = headlineContent,
    )
}

/**
 * A text field as a row of a [Group]: `label` over what is typed, `icon` before it.
 */
@Composable
fun FieldItem(
    value: String,
    onValueChange: (String) -> Unit,
    label: String?,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
    trailing: @Composable (() -> Unit)? = null,
    singleLine: Boolean = false,
    enabled: Boolean = true,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
    visualTransformation: VisualTransformation = VisualTransformation.None,
    containerColor: Color = MaterialTheme.colorScheme.surfaceContainer,
) = TextField(
    value = value,
    onValueChange = onValueChange,
    modifier = modifier.fillMaxWidth(),
    enabled = enabled,
    label = label?.let { { Text(it) } },
    // In line with an [Item]'s icon, a little further in.
    leadingIcon = icon?.let { { Box(Modifier.offset(x = 4.dp)) { Icon(it, null) } } },
    trailingIcon = trailing,
    visualTransformation = visualTransformation,
    keyboardOptions = keyboardOptions,
    keyboardActions = keyboardActions,
    singleLine = singleLine,
    shape = ITEM_SHAPE,
    colors = TextFieldDefaults.colors(
        focusedContainerColor = containerColor,
        unfocusedContainerColor = containerColor,
        disabledContainerColor = containerColor,
        focusedIndicatorColor = Color.Transparent,
        unfocusedIndicatorColor = Color.Transparent,
        disabledIndicatorColor = Color.Transparent,
    ),
)

/** One of `options` picked with connected toggle buttons sharing the room (Todav's). */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun Connected(options: List<String>, selected: Int, modifier: Modifier = Modifier, onSelect: (Int) -> Unit) = Row(
    modifier,
    horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween),
) {
    val haptics = LocalHapticFeedback.current
    options.forEachIndexed { i, o ->
        ToggleButton(
            checked = i == selected,
            onCheckedChange = {
                // A tick as another one is picked.
                if (i != selected) haptics.performHapticFeedback(HapticFeedbackType.SegmentTick)
                onSelect(i)
            },
            shapes = when (i) {
                0 -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                options.lastIndex -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
            },
            // Seen on a row's own colour too.
            colors = ToggleButtonDefaults.colors(
                containerColor = MaterialTheme.colorScheme.secondaryContainer,
                contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
            ),
            contentPadding = PaddingValues(horizontal = 8.dp),
            modifier = Modifier.weight(1f).semantics { role = Role.RadioButton },
        ) { Text(o, maxLines = 1, overflow = TextOverflow.Ellipsis) }
    }
}

/** An entry of a [Menu]; a `danger` one (Delete) in the error colour. */
class Entry(val label: String, val icon: ImageVector, val danger: Boolean = false, val action: () -> Unit)

/**
 * A menu of Material 3 Expressive groups, as Todav's task menu: each group a block of its own,
 * the empty ones left out. Picking an entry closes it.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun Menu(expanded: Boolean, onDismiss: () -> Unit, groups: List<List<Entry>>) = DropdownMenuPopup(expanded, onDismiss) {
    val shown = groups.filter { it.isNotEmpty() }
    shown.forEachIndexed { g, entries ->
        if (g > 0) Spacer(Modifier.height(MenuDefaults.GroupSpacing))
        DropdownMenuGroup(MenuDefaults.groupShape(g, shown.size)) {
            entries.forEachIndexed { i, e ->
                val color = if (e.danger) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface
                DropdownMenuItem(
                    onClick = {
                        onDismiss()
                        e.action()
                    },
                    text = { Text(e.label) },
                    shape = MenuDefaults.itemShape(i, entries.size).shape,
                    leadingIcon = { Icon(e.icon, null) },
                    colors = MenuDefaults.itemColors(textColor = color, leadingIconColor = color),
                )
            }
        }
    }
}

/** `content` (an icon button) with `label` as a tooltip when it is held or hovered. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Tip(label: String, content: @Composable () -> Unit) = TooltipBox(
    positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Below),
    tooltip = { PlainTooltip { Text(label) } },
    state = rememberTooltipState(),
    content = content,
)
