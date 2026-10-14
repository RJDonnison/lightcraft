package ai.storyteller.lightcraft.mobile;

import android.Manifest;
import android.content.ContentUris;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.graphics.Bitmap;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.provider.MediaStore;
import android.util.Size;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.ArrayList;
import java.util.List;

/** Paged, permission-aware MediaStore browser. Only previews are cached while browsing. */
final class GalleryBridge {
    private static final int PERMISSION = 0x4c46;
    private static final int PAGE = 48;
    private final LightCraftActivity activity;
    private final Object lock = new Object();
    private final List<String[]> pages = new ArrayList<>();
    private final List<String[]> opened = new ArrayList<>();
    private final java.util.concurrent.atomic.AtomicInteger generation = new java.util.concurrent.atomic.AtomicInteger();
    private volatile int waitingOffset = -1;
    private volatile boolean queried;

    GalleryBridge(LightCraftActivity activity) { this.activity = activity; }

    private boolean allowed() {
        if (Build.VERSION.SDK_INT >= 34) {
            return activity.checkSelfPermission(Manifest.permission.READ_MEDIA_IMAGES) == PackageManager.PERMISSION_GRANTED
                || activity.checkSelfPermission(Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED) == PackageManager.PERMISSION_GRANTED;
        }
        if (Build.VERSION.SDK_INT >= 33) return activity.checkSelfPermission(Manifest.permission.READ_MEDIA_IMAGES) == PackageManager.PERMISSION_GRANTED;
        return activity.checkSelfPermission(Manifest.permission.READ_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED;
    }

    void page(final int offset) {
        // Every page stays bounded, but the gallery itself need not fit an arbitrary
        // maximum count. Reject an offset that could overflow when adding PAGE.
        if (offset < 0 || offset > Integer.MAX_VALUE - PAGE) {
            enqueue(pages, new String[] {"error", "Photo page is out of range"});
            return;
        }
        queried = true;
        final int request = generation.incrementAndGet();
        synchronized (lock) { pages.clear(); }
        if (!allowed()) {
            waitingOffset = offset;
            activity.runOnUiThread(new Runnable() {
                @Override public void run() {
                    String[] permissions = Build.VERSION.SDK_INT >= 34
                        ? new String[] {Manifest.permission.READ_MEDIA_IMAGES, Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED}
                        : new String[] {Build.VERSION.SDK_INT >= 33 ? Manifest.permission.READ_MEDIA_IMAGES : Manifest.permission.READ_EXTERNAL_STORAGE};
                    activity.requestPermissions(permissions, PERMISSION);
                }
            });
            return;
        }
        new Thread(new Runnable() {
            @Override public void run() {
                try {
                    List<String> result = new ArrayList<>();
                    result.add("page"); result.add(Integer.toString(offset)); result.add("0");
                    Uri collection = MediaStore.Images.Media.EXTERNAL_CONTENT_URI;
                    String[] cols = {MediaStore.Images.Media._ID, MediaStore.Images.Media.DISPLAY_NAME};
                    String order = MediaStore.Images.Media.DATE_TAKEN + " DESC, " + MediaStore.Images.Media._ID + " DESC";
                    Cursor page;
                    if (Build.VERSION.SDK_INT >= 30) {
                        Bundle args = new Bundle();
                        args.putInt(android.content.ContentResolver.QUERY_ARG_LIMIT, PAGE);
                        args.putInt(android.content.ContentResolver.QUERY_ARG_OFFSET, offset);
                        args.putString(android.content.ContentResolver.QUERY_ARG_SQL_SORT_ORDER, order);
                        page = activity.getContentResolver().query(collection, cols, args, null);
                    } else {
                        // API 26–29 providers accept a bounded SQL LIMIT in the sort order.
                        page = activity.getContentResolver().query(collection, cols, null, null, order + " LIMIT " + PAGE + " OFFSET " + offset);
                    }
                    try (Cursor cursor = page) {
                        if (cursor != null) {
                            int count = 0;
                            while (cursor.moveToNext() && count < PAGE) {
                                long id = cursor.getLong(0);
                                String name = cursor.getString(1);
                                Uri uri = ContentUris.withAppendedId(collection, id);
                                File thumbnail = new File(activity.getCacheDir(), "gallery-" + id + ".jpg");
                                try {
                                    if (!thumbnail.isFile()) {
                                        Bitmap bitmap = Build.VERSION.SDK_INT >= 29
                                            ? activity.getContentResolver().loadThumbnail(uri, new Size(192, 192), null)
                                            : MediaStore.Images.Thumbnails.getThumbnail(activity.getContentResolver(), id, MediaStore.Images.Thumbnails.MINI_KIND, null);
                                        if (bitmap != null) {
                                            try (OutputStream out = new FileOutputStream(thumbnail)) { bitmap.compress(Bitmap.CompressFormat.JPEG, 80, out); }
                                            bitmap.recycle();
                                        }
                                    }
                                } catch (Exception e) { android.util.Log.w("LightCraft", "gallery thumbnail: " + e); }
                                result.add(Long.toString(id));
                                result.add(name == null ? "Photo" : name);
                                result.add(thumbnail.isFile() ? thumbnail.getAbsolutePath() : "");
                                count++;
                            }
                            result.set(2, count == PAGE ? "1" : "0");
                        }
                    }
                    enqueuePage(request, result.toArray(new String[0]));
                } catch (SecurityException e) { enqueuePage(request, new String[] {"permission"}); }
                catch (Exception e) { enqueuePage(request, new String[] {"error", e.toString()}); }
            }
        }).start();
    }

    void permissionResult(int request) {
        if (request != PERMISSION) return;
        int offset = waitingOffset;
        waitingOffset = -1;
        if (allowed()) page(Math.max(0, offset));
        else enqueue(pages, new String[] {"permission"});
    }

    void resumed() {
        if (!queried) return;
        if (allowed()) page(0);
        else {
            generation.incrementAndGet();
            synchronized (lock) {
                pages.clear();
                pages.add(new String[] {"permission"});
            }
        }
    }

    void open(final String id, final String target) {
        synchronized (lock) { opened.clear(); }
        new Thread(new Runnable() {
            @Override public void run() {
                try {
                    long value = Long.parseLong(id);
                    if (value < 0 || !allowed()) throw new SecurityException("Photo access has changed. Allow access and retry.");
                    Uri uri = ContentUris.withAppendedId(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, value);
                    String name = "photo-" + id;
                    try (Cursor cursor = activity.getContentResolver().query(uri, new String[] {MediaStore.Images.Media.DISPLAY_NAME}, null, null, null)) {
                        if (cursor != null && cursor.moveToFirst() && cursor.getString(0) != null) name = cursor.getString(0);
                    }
                    String type = activity.getContentResolver().getType(uri);
                    boolean heif = "image/heic".equals(type) || "image/heif".equals(type);
                    String ext = type == null ? null : android.webkit.MimeTypeMap.getSingleton().getExtensionFromMimeType(type);
                    if (ext != null && !ext.matches("[a-zA-Z0-9]{1,8}")) ext = null;
                    if (heif) ext = "jpg";
                    int dot = name.lastIndexOf('.');
                    if (ext == null && dot >= 0 && name.length() - dot <= 9) {
                        String suffix = name.substring(dot + 1).toLowerCase(java.util.Locale.ROOT);
                        if (suffix.matches("[a-z0-9]{1,8}")) ext = suffix;
                    }
                    if (ext == null) ext = "jpg";
                    File dir = new File(target);
                    if (!dir.isDirectory() && !dir.mkdirs()) throw new java.io.IOException("Cannot create photo library directory");
                    // Stable, path-safe ID: renaming the gallery item cannot create a second copy.
                    File file = new File(dir, "device-" + id + "." + ext);
                    if (!file.isFile()) {
                        File temp = new File(dir, id + "-pending");
                        try {
                            if (heif && Build.VERSION.SDK_INT >= 28) {
                                Bitmap bitmap = android.graphics.ImageDecoder.decodeBitmap(android.graphics.ImageDecoder.createSource(activity.getContentResolver(), uri));
                                try (OutputStream output = new FileOutputStream(temp)) {
                                    if (!bitmap.compress(Bitmap.CompressFormat.JPEG, 95, output)) throw new java.io.IOException("Cannot convert HEIF photo");
                                } finally { bitmap.recycle(); }
                            } else {
                                try (InputStream input = activity.getContentResolver().openInputStream(uri);
                                     OutputStream output = new FileOutputStream(temp)) {
                                    if (input == null) throw new java.io.IOException("Photo is unavailable");
                                    byte[] buffer = new byte[65536];
                                    for (int n; (n = input.read(buffer)) > 0;) output.write(buffer, 0, n);
                                }
                            }
                            if (!temp.renameTo(file)) throw new java.io.IOException("Could not finish copying photo");
                        } finally { if (temp.exists()) temp.delete(); }
                    }
                    enqueue(opened, new String[] {"ok", file.getAbsolutePath()});
                } catch (OutOfMemoryError e) { enqueue(opened, new String[] {"error", "Not enough memory to convert this photo"}); }
                catch (Exception e) { enqueue(opened, new String[] {"error", e.toString()}); }
            }
        }).start();
    }

    private void enqueue(List<String[]> queue, String[] value) { synchronized (lock) { queue.add(value); } }
    private void enqueuePage(int request, String[] value) {
        synchronized (lock) {
            if (generation.get() == request) pages.add(value);
        }
    }
    String[] takePage() { synchronized (lock) { return pages.isEmpty() ? null : pages.remove(0); } }
    String[] takeOpen() { synchronized (lock) { return opened.isEmpty() ? null : opened.remove(0); } }
}
