package io.github.petrotta.mercurio.plugins;

import io.github.petrotta.mercurio.Application;
import io.github.petrotta.mercurio.build.Project;
import io.github.petrotta.mercurio.plugins.type.BasePlugin;
import io.github.petrotta.mercurio.plugins.type.LanguagePlugin;
import io.github.petrotta.mercurio.plugins.type.TaskPlugin;
import io.github.petrotta.mercurio.utils.ZipClassLoader;
import io.github.petrotta.mercurio.utils.ZipUtils;
//import java.io.*;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.lang.annotation.Annotation;
import java.lang.reflect.InvocationTargetException;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.*;
import java.util.*;
import java.util.zip.ZipFile;
import java.util.zip.ZipInputStream;
import java.util.zip.ZipOutputStream;

import static io.github.petrotta.mercurio.Application.console;



public class PluginManager {

    Map<String, BasePlugin> basePlugins = new HashMap<>();


    public LanguagePlugin getLanguagePluginClass(String key) {

        if(basePlugins.containsKey(key) && basePlugins.get(key) instanceof LanguagePlugin) {
            return (LanguagePlugin) basePlugins.get(key);
        }
        return null;

    }
    public TaskPlugin getTaskPluginClass(String key) {

        if(basePlugins.containsKey(key) && basePlugins.get(key) instanceof TaskPlugin) {
            return (TaskPlugin) basePlugins.get(key);
        }
        return null;

    }

    public void index() throws Exception {

        console("loading plugins");


//        File pluginsDir = Application.getPluginsDir();
//
//        File[] fileList = pluginsDir.listFiles();
        try (DirectoryStream<Path> stream = Files.newDirectoryStream(Application.getPluginsDir().toPath())) {
            for (Path entry: stream) {
                if(entry.getFileName().toString().endsWith(".zip")) {
                    loadPlugin(entry);

                }
            }
        }



//        for(File pluginDirs : fileList!=null ? fileList : new File[0]) {
//            File libs = new File(pluginDirs, "libs");
//            if(pluginDirs.isDirectory() && libs.isDirectory()) {
//
//
//                for(File jar : libs.listFiles((file)->{return file.isFile() && file.getName().endsWith(".jar");})) {
//
//
//
//                }
//            }
//        }
    }

    public void run(String task, Project project) {
        this.getTaskPluginClass(task).run(project);
    }

    // path to zip plugin
    private void loadPlugin(Path zipPath) throws Exception {


        try (FileSystem fs = FileSystems.newFileSystem(zipPath)) {
            Path libDir = fs.getPath("lib");
            if(libDir != null) {
                try (DirectoryStream<Path> stream = Files.newDirectoryStream(libDir)) {
                    for (Path entry: stream) {
                        if(entry.getFileName().toString().endsWith(".jar")) {
                            console("loading jar " + entry.getFileName().toString());
                            loadJar(entry);
                        }
                    }
                }
            }


        }


//        URLClassLoader child = new URLClassLoader(new URL[]{jar.toURI().toURL()}, this.getClass().getClassLoader());
//
//        List<String> allClassesInJar = ZipUtils.getAllClassesInJar(jar);
//        for(String className : allClassesInJar) {
//            Class classToLoad = child.loadClass(className);
//            if(classToLoad.isAnnotationPresent(PluginAnnotation.class) && TaskPlugin.class.isAssignableFrom(classToLoad)) {
//                for(Annotation annotation : classToLoad.getAnnotations()) {
//                    if(annotation.annotationType().equals(PluginAnnotation.class)) {
//                        PluginAnnotation pluginAnnotation = (PluginAnnotation) annotation;
//
//                        console("Loaded plugin " + classToLoad.getName());
//                        BasePlugin plugin = (BasePlugin) classToLoad.getConstructor().newInstance();
//
//                        basePlugins.put(pluginAnnotation.name(), plugin);
//                        plugin.init();
//                    }
//                }
//            }
//        }
    }

    private void loadJar(Path jar) throws IOException, ClassNotFoundException, NoSuchMethodException, InvocationTargetException, InstantiationException, IllegalAccessException {


        ZipClassLoader zipClassLoader = new ZipClassLoader(new URL[] {jar.toUri().toURL()}, jar);
        for(String className : zipClassLoader.getClasses()) {
            Class classToLoad = zipClassLoader.loadClass(className);
            if(classToLoad.isAnnotationPresent(PluginAnnotation.class) && TaskPlugin.class.isAssignableFrom(classToLoad)) {
                for(Annotation annotation : classToLoad.getAnnotations()) {
                    if(annotation.annotationType().equals(PluginAnnotation.class)) {
                        PluginAnnotation pluginAnnotation = (PluginAnnotation) annotation;

                        console("Loaded plugin " + classToLoad.getName());
                        BasePlugin plugin = (BasePlugin) classToLoad.getConstructor().newInstance();

                        basePlugins.put(pluginAnnotation.name(), plugin);
                        plugin.init();
                    }
                }
            }

        }


//        new URLClassLoader()
//        URLClassLoader child = new URLClassLoader(new URL[] { jar.toUri().toURL()} );
//
//        ZipInputStream zis = new ZipInputStream(Files.newInputStream(jar));
//
//        List<String> allClassesInJar = ZipUtils.getAllClassesInJar(zis);
//        for(String className : allClassesInJar) {
//            Class classToLoad = child.loadClass(className);
//            if(classToLoad.isAnnotationPresent(PluginAnnotation.class) && TaskPlugin.class.isAssignableFrom(classToLoad)) {
//                for(Annotation annotation : classToLoad.getAnnotations()) {
//                    if(annotation.annotationType().equals(PluginAnnotation.class)) {
//                        PluginAnnotation pluginAnnotation = (PluginAnnotation) annotation;
//
//                        console("Loaded plugin " + classToLoad.getName());
//                        BasePlugin plugin = (BasePlugin) classToLoad.getConstructor().newInstance();
//
//                        basePlugins.put(pluginAnnotation.name(), plugin);
//                        plugin.init();
//                    }
//                }
//            }
//        }


    }



}
