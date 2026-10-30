#define _GNU_SOURCE
#include <gtk/gtk.h>
#include <gdk/gdk.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <signal.h>

static char **g_uris = NULL;
static GtkWidget *g_win = NULL;
static GtkTargetList *g_target_list = NULL;
static int g_attempts = 0;

static void on_drag_data_get(GtkWidget *widget, GdkDragContext *context,
                             GtkSelectionData *data, guint info, guint time,
                             gpointer user_data) {
    char **uris = (char **)user_data;
    if (info == 0) { // text/uri-list (Wayland, Firefox, Chrome, Nemo)
        gtk_selection_data_set_uris(data, uris);
    } else if (info == 1) { // x-special/gnome-copied-files (GTK / Wayland / Nemo)
        GString *s = g_string_new("copy\n");
        for (int i = 0; uris[i]; i++) {
            g_string_append_printf(s, "%s\n", uris[i]);
        }
        GdkAtom atom = gdk_atom_intern("x-special/gnome-copied-files", FALSE);
        gtk_selection_data_set(data, atom, 8, (const guchar *)s->str, s->len);
        g_string_free(s, TRUE);
    } else if (info == 4) { // DownloadURL (Browser direct file save/upload)
        if (uris[0]) {
            char *filename = g_path_get_basename(uris[0]);
            GString *s = g_string_new("");
            g_string_append_printf(s, "application/octet-stream:%s:%s", filename, uris[0]);
            GdkAtom atom = gdk_atom_intern("DownloadURL", FALSE);
            gtk_selection_data_set(data, atom, 8, (const guchar *)s->str, s->len);
            g_free(filename);
            g_string_free(s, TRUE);
        }
    } else { // text/plain, UTF8_STRING, STRING (Terminal / Text editors)
        GString *s = g_string_new("");
        for (int i = 0; uris[i]; i++) {
            g_string_append_printf(s, "%s\n", uris[i]);
        }
        gtk_selection_data_set_text(data, s->str, s->len);
        g_string_free(s, TRUE);
    }
}

static void on_drag_end(GtkWidget *widget, GdkDragContext *context, gpointer user_data) {
    gtk_main_quit();
}

static void on_drag_failed(GtkWidget *widget, GdkDragContext *context, GtkDragResult result, gpointer user_data) {
    gtk_main_quit();
}

static gboolean timeout_killer(gpointer user_data) {
    gtk_main_quit();
    return G_SOURCE_REMOVE;
}

static gboolean try_begin_drag_cb(gpointer user_data) {
    g_attempts++;
    GdkDisplay *disp = gdk_display_get_default();
    gint x = 0, y = 0;
    if (disp) {
        GdkSeat *seat = gdk_display_get_default_seat(disp);
        GdkDevice *pointer = seat ? gdk_seat_get_pointer(seat) : NULL;
        if (pointer) {
            gdk_device_get_position(pointer, NULL, &x, &y);
            gtk_window_move(GTK_WINDOW(g_win), x - 10, y - 10);
        }
    }

    GdkDragContext *ctx = gtk_drag_begin_with_coordinates(
        g_win,
        g_target_list,
        GDK_ACTION_COPY | GDK_ACTION_MOVE,
        1,
        NULL,
        x, y
    );

    if (ctx) {
        return G_SOURCE_REMOVE;
    }

    if (g_attempts >= 10) {
        gtk_main_quit();
        return G_SOURCE_REMOVE;
    }

    return G_SOURCE_CONTINUE;
}

int main(int argc, char **argv) {
    if (argc < 2) return 0;
    signal(SIGTERM, SIG_DFL);
    signal(SIGINT, SIG_DFL);

    gtk_init(&argc, &argv);

    g_uris = malloc(sizeof(char*) * argc);
    for (int i = 1; i < argc; i++) {
        char *path = argv[i];
        char *uri = g_filename_to_uri(path, NULL, NULL);
        g_uris[i-1] = uri ? uri : g_strdup(path);
    }
    g_uris[argc-1] = NULL;

    g_win = gtk_window_new(GTK_WINDOW_POPUP);
    gtk_window_set_default_size(GTK_WINDOW(g_win), 20, 20);

    GtkTargetEntry targets[] = {
        { "text/uri-list", 0, 0 },
        { "x-special/gnome-copied-files", 0, 1 },
        { "text/plain", 0, 2 },
        { "UTF8_STRING", 0, 3 },
        { "DownloadURL", 0, 4 },
        { "STRING", 0, 5 }
    };
    g_target_list = gtk_target_list_new(targets, 6);

    g_signal_connect(g_win, "drag-data-get", G_CALLBACK(on_drag_data_get), g_uris);
    g_signal_connect(g_win, "drag-end", G_CALLBACK(on_drag_end), NULL);
    g_signal_connect(g_win, "drag-failed", G_CALLBACK(on_drag_failed), NULL);

    gtk_widget_show_all(g_win);

    // Timeout: Max 15 seconds to completely prevent zombie processes
    g_timeout_add_seconds(15, timeout_killer, NULL);

    GdkDisplay *disp = gdk_display_get_default();
    gint x = 0, y = 0;
    if (disp) {
        GdkSeat *seat = gdk_display_get_default_seat(disp);
        GdkDevice *pointer = seat ? gdk_seat_get_pointer(seat) : NULL;
        if (pointer) {
            gdk_device_get_position(pointer, NULL, &x, &y);
            gtk_window_move(GTK_WINDOW(g_win), x - 10, y - 10);
        }
    }

    GdkDragContext *ctx = gtk_drag_begin_with_coordinates(
        g_win,
        g_target_list,
        GDK_ACTION_COPY | GDK_ACTION_MOVE,
        1,
        NULL,
        x, y
    );

    if (!ctx) {
        g_timeout_add(20, try_begin_drag_cb, NULL);
    }

    gtk_main();

    if (g_target_list) {
        gtk_target_list_unref(g_target_list);
    }
    return 0;
}

