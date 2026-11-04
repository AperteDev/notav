package dev.aperte.notav

import dev.aperte.notav.core.Note
import java.text.Collator

/** A row of the notes' tree. Accounts have rows while more than one is shown. */
sealed interface Item {
    /** The account and the folder this row's new notes go to: its own for an account and a
     * folder (the top level for an account), a note's folder. */
    val place: Pair<Long, String>

    data class Account(val id: Long) : Item {
        override val place get() = id to ""
    }

    data class Folder(val account: Long, val path: String) : Item {
        override val place get() = account to path
        val name get() = path.substringAfterLast('/')
    }

    data class NoteRow(val note: Note) : Item {
        override val place get() = note.account to note.folder
    }
}

data class Row(val depth: Int, val item: Item)

/** A folder's key among the open ones (`expanded_folders`, as on Linux): the account's id, ":",
 * its path. An account's is its id and ":", there while it is closed: accounts start open. */
fun folderKey(account: Long, folder: String) = "$account:$folder"

/**
 * The tree's rows, as the Linux app's (`window.rs`, `tree`): with more than one account, each
 * account's (open unless closed), then what is in it. In a folder its folders, then its notes
 * (favorites first), by name in the locale's order; what is in a folder only while it is open.
 */
fun tree(
    accounts: List<Pair<Long, List<String>>>,
    notes: List<Note>,
    expanded: Set<String>,
    collator: Collator = Collator.getInstance(),
): List<Row> {
    val byName = compareBy<String, String>(collator) { it }
    val out = mutableListOf<Row>()
    fun add(account: Long, folder: String, depth: Int, folders: List<String>) {
        val subs = folders.filter { it.substringBeforeLast('/', "") == folder }
            .sortedWith(compareBy(byName) { it.substringAfterLast('/') })
        for (f in subs) {
            out += Row(depth, Item.Folder(account, f))
            if (folderKey(account, f) in expanded) add(account, f, depth + 1, folders)
        }
        notes.filter { it.account == account && it.folder == folder }
            .sortedWith(compareByDescending<Note> { it.favorite }.thenBy(byName) { it.title })
            .forEach { out += Row(depth, Item.NoteRow(it)) }
    }
    if (accounts.size == 1) {
        add(accounts[0].first, "", 0, accounts[0].second)
    } else {
        for ((account, folders) in accounts) {
            out += Row(0, Item.Account(account))
            if (folderKey(account, "") !in expanded) add(account, "", 1, folders)
        }
    }
    return out
}

/** Whether moving `item` (a note or folder) into `folder` of its account moves anything: not
 * where it is already, nor a folder into itself. As the Linux app's `moves`. */
fun moves(item: Item, folder: String): Boolean = when (item) {
    is Item.NoteRow -> item.note.folder != folder
    is Item.Folder -> item.path.substringBeforeLast('/', "") != folder &&
        folder != item.path && !folder.startsWith("${item.path}/")
    is Item.Account -> false
}

/** How many notes are in `folder` of `account` and its folders ("": the whole account). */
fun notesIn(notes: List<Note>, account: Long, folder: String) = notes.count {
    it.account == account && (folder.isEmpty() || it.folder == folder || it.folder.startsWith("$folder/"))
}
