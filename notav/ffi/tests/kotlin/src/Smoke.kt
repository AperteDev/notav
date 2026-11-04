// A run through the bindings on a fresh database: a local account, notes and folders, search
// (FTS5 in the bundled SQLite), undo, Editor mode's Markdown in UTF-16 positions, translations,
// errors and callbacks.

import dev.aperte.notav.core.*
import kotlin.io.path.createTempDirectory

fun main() {
    val dir = createTempDirectory("notav-ffi").toFile()
    val n = Notav.open(dir.path)
    var changes = 0
    n.subscribe(object : Listener {
        override fun changed(paths: List<String>) { changes++ }
        override fun settingsChanged() {}
        override fun progress(account: Long, done: UInt, total: UInt) {}
    })

    val account = n.addAccount(AccountKind.LOCAL, "", "", "")
    check(n.accounts().single().kind == AccountKind.LOCAL)

    // Notes and folders.
    n.addFolder(account, "Work")
    val note = n.addNote(account, "Work", "Plan", "# Plan\n\n- first")
    check(note.path == "Work/Plan.md" && note.folder == "Work") { "note: $note" }
    n.setText(note.id, "# Plan\n\nBuy żółte kwiaty")
    check(n.noteText(note.id) == "# Plan\n\nBuy żółte kwiaty")
    check(n.folders(account) == listOf("Work"))
    check(n.search("zolte").single().id == note.id) { "search ignores accents" }

    // A move, a delete, and the delete undone.
    val moved = n.moveNote(note.id, "", "Plans")
    check(moved.path == "Plans.md")
    n.deleteNote(note.id)
    check(n.note(note.id) == null)
    check(n.undo(null))
    check(n.notes().any { it.title == "Plans" })

    // Errors come as NotavException.
    try {
        n.addAccount(AccountKind.LOCAL, "", "", "")
        error("a second local account")
    } catch (e: NotavException.Exists) {
    }

    // Editor mode: positions in UTF-16 code units, an emoji (two of them) before the text.
    val text = "😀 **bold**"
    val strong = noteSpans(text).first { it.style == NoteStyle.Strong }
    check(text.substring(strong.start.toInt(), strong.end.toInt()) == "**bold**") { "span: $strong" }
    val list = "😀\n- item"
    val enter = noteEnter(list, list.length.toUInt()).single()
    check(enter.text.contains("- ") && enter.start.toInt() == list.length) { "enter: $enter" }
    val link = noteLinkAt("😀 [[Plans]]", 5u)
    check(link is NoteLink.Note && link.target == "Plans" && link.wiki) { "link: $link" }
    check(noteHeadings("😀\n## Two").single().let { it.level == 2.toUByte() && it.at == 3u })

    // Translations from the Polish catalog (made by scripts/bindings.sh).
    val pl = Translations(java.io.File("../../target/bindings/pl.mo").readBytes())
    check(pl.gettext("Today’s Note") == "Dzisiejsza notatka")
    check(pl.ngettext("{count} match", "{count} matches", 5uL) == "{count} dopasowań")
    check(pl.pgettext("account", "Off") == "Wyłączone")
    check(Translations.english().ngettext("{count} match", "{count} matches", 1uL) == "{count} match")

    check(changes > 0) { "no change events" }
    println("Kotlin bindings: ok ($changes change events)")
    dir.deleteRecursively()
}
