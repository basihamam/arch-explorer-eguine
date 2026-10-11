#include <gtk/gtk.h>
#include <stdio.h>
#include <stdlib.h>

static void on_drag_data_get(GtkWidget *widget, GdkDragContext *context,
                             GtkSelectionData *data, guint info, guint time,
                             gpointer user_data) {
    char **uris = (char **)user_data;
    if (info == 0) {
        gtk_selection_data_set_uris(data, uris);
    } else if (info == 1) {
        GString *s = g_string_new("copy\n");
        for (int i = 0; uris[i]; i++) {
            g_string_append_printf(s, "%s\n", uris[i]);
        }
        GdkAtom atom = gdk_atom_intern("x-special/gnome-copied-files", FALSE);
        gtk_selection_data_set(data, atom, 8, (const guchar *)s->str, s->len);
        g_string_free(s, TRUE);
    } else {
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

int main(int argc, char **argv) {
    if (argc < 2) return 0;
    gtk_init(&argc, &argv);

    char **uris = malloc(sizeof(char*) * argc);
    for (int i = 1; i < argc; i++) {
        char *path = argv[i];
        char *uri = g_filename_to_uri(path, NULL, NULL);
        uris[i-1] = uri ? uri : g_strdup(path);
    }
    uris[argc-1] = NULL;

    GtkWidget *win = gtk_window_new(GTK_WINDOW_POPUP);
    gtk_window_set_default_size(GTK_WINDOW(win), 32, 32);

    GdkDisplay *disp = gdk_display_get_default();
    if (!disp) return 1;
    GdkSeat *seat = gdk_display_get_default_seat(disp);
    GdkDevice *pointer = gdk_seat_get_pointer(seat);
    gint x = 0, y = 0;
    gdk_device_get_position(pointer, NULL, &x, &y);

    gtk_window_move(GTK_WINDOW(win), x - 16, y - 16);
    gtk_widget_show_all(win);

    GtkTargetEntry targets[] = {
        { "text/uri-list", 0, 0 },
        { "x-special/gnome-copied-files", 0, 1 },
        { "text/plain", 0, 2 },
        { "UTF8_STRING", 0, 3 }
    };
    GtkTargetList *target_list = gtk_target_list_new(targets, 4);

    g_signal_connect(win, "drag-data-get", G_CALLBACK(on_drag_data_get), uris);
    g_signal_connect(win, "drag-end", G_CALLBACK(on_drag_end), NULL);
    g_signal_connect(win, "drag-failed", G_CALLBACK(on_drag_failed), NULL);

    GdkDragContext *ctx = gtk_drag_begin_with_coordinates(
        win,
        target_list,
        GDK_ACTION_COPY | GDK_ACTION_MOVE,
        1,
        NULL,
        x, y
    );
    gtk_target_list_unref(target_list);

    if (ctx) {
        gtk_main();
    }
    return 0;
}
