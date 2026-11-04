package dev.aperte.notav

import dev.aperte.notav.core.Note
import java.text.Collator
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The Linux app's tree tests (`window.rs`), for the Kotlin tree. */
class TreeTest {
    private fun note(path: String, account: Long = 1, favorite: Boolean = false) = Note(
        id = 0,
        account = account,
        path = path,
        title = path.substringAfterLast('/').removeSuffix(".md"),
        folder = path.substringBeforeLast('/', ""),
        modified = 0,
        pending = false,
        favorite = favorite,
    )

    private val collator = Collator.getInstance(Locale.ENGLISH)

    private fun names(accounts: List<Pair<Long, List<String>>>, notes: List<Note>, expanded: Set<String>) =
        tree(accounts, notes, expanded, collator).map { r ->
            r.depth to when (val i = r.item) {
                is Item.Account -> "#${i.id}"
                is Item.Folder -> "${i.path}/"
                is Item.NoteRow -> i.note.title
            }
        }

    @Test
    fun accountsHaveRowsWhenMoreThanOneShows() {
        val notes = listOf(note("a.md"), note("b.md", account = 2))
        assertEquals(listOf(0 to "a"), names(listOf(1L to listOf()), notes, setOf()))
        val both = listOf(1L to listOf(), 2L to listOf("Work"))
        assertEquals(
            listOf(0 to "#1", 1 to "a", 0 to "#2", 1 to "Work/", 1 to "b"),
            names(both, notes, setOf()),
        )
        assertEquals(listOf(0 to "#1", 1 to "a", 0 to "#2"), names(both, notes, setOf("2:")))
    }

    @Test
    fun folderComeFirstAndOnlyOpenOnesShowTheirContents() {
        val folders = listOf("Work", "Work/Old", "archive")
        val notes = listOf("b.md", "A.md", "Work/Plan.md", "Work/Old/x.md", "archive/y.md").map { note(it) }
        val one = listOf(1L to folders)
        // Case does not decide the order: "archive" before "Work", "A" before "b".
        assertEquals(listOf(0 to "archive/", 0 to "Work/", 0 to "A", 0 to "b"), names(one, notes, setOf()))
        assertEquals(
            listOf(0 to "archive/", 0 to "Work/", 1 to "Work/Old/", 2 to "x", 1 to "Plan", 0 to "A", 0 to "b"),
            names(one, notes, setOf(folderKey(1, "Work"), folderKey(1, "Work/Old"))),
        )
    }

    @Test
    fun favoritesComeFirstInTheirFolder() {
        val notes = listOf(note("a.md"), note("b.md"), note("c.md", favorite = true))
        assertEquals(listOf("c", "a", "b"), names(listOf(1L to listOf()), notes, setOf()).map { it.second })
    }

    @Test
    fun foldersCountTheirNotesAndTheirFoldersNotes() {
        val notes = listOf("a.md", "Work/Plan.md", "Work/Old/x.md", "Workshop/y.md").map { note(it) } +
            note("Work/Other.md", account = 2)
        assertEquals(2, notesIn(notes, 1, "Work"))
        assertEquals(1, notesIn(notes, 1, "Work/Old"))
        assertEquals(1, notesIn(notes, 1, "Workshop"))
        assertEquals(1, notesIn(notes, 2, "Work"))
        assertEquals(4, notesIn(notes, 1, ""))
    }

    @Test
    fun movesThatMoveNothingOrIntoItselfAreRefused() {
        val folder = { f: String -> Item.Folder(1, f) }
        assertTrue(moves(Item.NoteRow(note("Work/Plan.md")), ""))
        assertFalse(moves(Item.NoteRow(note("Work/Plan.md")), "Work"))
        assertTrue(moves(folder("Work/Old"), ""))
        assertTrue(moves(folder("Work"), "Archive"))
        assertFalse(moves(folder("Work/Old"), "Work"))
        assertFalse(moves(folder("Work"), "Work"))
        assertFalse(moves(folder("Work"), "Work/Old"))
        assertTrue(moves(folder("Work"), "Workshop"))
    }
}
