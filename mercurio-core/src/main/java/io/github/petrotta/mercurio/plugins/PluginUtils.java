package io.github.petrotta.mercurio.plugins;

import io.github.petrotta.mercurio.Application;
import io.github.petrotta.mercurio.build.Project;
import io.github.petrotta.mercurio.utils.ZipUtils;
import java.io.*;
import java.lang.annotation.Annotation;
import java.net.URL;
import java.net.URLClassLoader;
import java.util.*;
import static io.github.petrotta.mercurio.Application.console;



public class PluginUtils {

    Map<String, Plugin> classes = new HashMap<>();

    public Plugin getPluginClass(String key) {
        return classes.get(key);
    }

    public void index() throws Exception {

        console("loading plugins");

        File pluginsDir = Application.getPluginsDir();

        File[] fileList = pluginsDir.listFiles();

        for(File pluginDirs : fileList!=null ? fileList : new File[0]) {
            File libs = new File(pluginDirs, "libs");
            if(pluginDirs.isDirectory() && libs.isDirectory()) {

                //File descriptors = new File(pluginDirs, "plugin.xml");
                //PluginXML pluginXML = readPluginXMLFile(descriptors);

                for(File jar : libs.listFiles((file)->{return file.isFile() && file.getName().endsWith(".jar");})) {
                    URLClassLoader child = new URLClassLoader(new URL[]{jar.toURI().toURL()}, this.getClass().getClassLoader());
                    //Class classToLoad = Class.forName(pluginXML.getEntryClass(), true, child);
                    //console(classToLoad.toString());

                    List<String> allClassesInJar = ZipUtils.getAllClassesInJar(jar);
                    for(String className : allClassesInJar) {
                        Class classToLoad = child.loadClass(className);
                        if(classToLoad.isAnnotationPresent(PluginAnnotation.class) && Plugin.class.isAssignableFrom(classToLoad)) {
                            for(Annotation annotation : classToLoad.getAnnotations()) {
                                if(annotation.annotationType().equals(PluginAnnotation.class)) {
                                    PluginAnnotation pluginAnnotation = (PluginAnnotation) annotation;

                                    console("Loaded plugin " + classToLoad.getName());
                                    Plugin plugin = (Plugin) classToLoad.getConstructor().newInstance();

                                    classes.put(pluginAnnotation.name(), plugin);
                                    plugin.init();
                                }
                            }
                        }
                    }


                }
            }
        }
    }

    public void run(String task, Project project) {
        this.getPluginClass(task).run(project);
    }

//    public static PluginXML readPluginXMLFile(File file) throws IOException, SAXException {
//        XmlMapper xmlMapper = new XmlMapper();
//        PluginXML pluginXML = xmlMapper.readValue(file, PluginXML.class);
//        return pluginXML;
//    }



}
