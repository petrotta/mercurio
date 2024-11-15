package io.github.petrotta.mercurio.utils;

import lombok.Getter;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

public final class ZipClassLoader extends URLClassLoader {

    private final Map<String, byte[]> classes;

    public ZipClassLoader(URL[] urls, final Path zipFile) throws IOException {
        super(urls);
        classes = loadZip(zipFile);
    }

    public Set<String> getClasses() {
        return classes.keySet();
    }

    @Override
    public Class<?> findClass(String name) throws ClassNotFoundException {
        final byte[] bytes = classes.get(name);
        if(bytes != null) return defineClass(name, bytes, 0, bytes.length);
        return super.findClass(name);
    }

    private final static String JAR_EXTENSION = ".jar";

    private static Map<String, byte[]> loadZip(final Path file) throws IOException {
        final byte[]          bytes = Files.readAllBytes(file);
        final ByteArrayOutputStream baos  = new ByteArrayOutputStream();
        baos.write(bytes);
        return unzip(baos);
    }

    private static  Map<String, byte[]> unzip(final ByteArrayOutputStream baos) {
        final Map<String, byte[]> result = new HashMap<String, byte[]>();
        try(final ZipInputStream in = new ZipInputStream(new ByteArrayInputStream(baos.toByteArray()))) {
            ZipEntry entry;
            while ((entry = in.getNextEntry()) != null) {
                final ByteArrayOutputStream os = new ByteArrayOutputStream();
                if (!entry.isDirectory()) {
                    os.write(in.readAllBytes());
                    if (entry.getName().toLowerCase().endsWith(JAR_EXTENSION)) {
                        result.putAll(unzip(os));
                    } else if (entry.getName().toLowerCase().endsWith(".class")) {
                        result.put(entry.getName().replaceAll("/", ".").substring(0, entry.getName().length() - 6), os.toByteArray());
                    }
                }
            }
            return result;
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }
}

