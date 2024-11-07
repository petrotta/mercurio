package io.github.petrotta.mercurio.plugins;

import com.fasterxml.jackson.dataformat.xml.XmlMapper;
import io.github.petrotta.mercurio.Application;
import org.xml.sax.SAXException;


import java.io.*;
import java.lang.annotation.Annotation;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.*;
import java.util.*;
import java.util.stream.Collectors;

import static io.github.petrotta.mercurio.Application.console;



public class PluginUtils {

    Map<String, Class> classes = new HashMap();

    public Class getPluginClass(String key) {
        return classes.get(key);
    }

    public void index() throws Exception {

        console("loading plugins");

        File pluginsDir = Application.getPluginsDir();


        for(File pluginDirs : pluginsDir.listFiles()) {

            if(pluginDirs.isDirectory()) {
                File libs = new File(pluginDirs, "libs");
                File descriptors = new File(pluginDirs, "plugin.xml");
                PluginXML pluginXML = readPluginXMLFile(descriptors);

                for(File jar : libs.listFiles((file)->{return file.isFile() && file.getName().endsWith(".jar");})) {
                    URLClassLoader child = new URLClassLoader(new URL[]{jar.toURI().toURL()}, null);
                    Class classToLoad = Class.forName(pluginXML.getEntryClass(), true, child);
                    console(classToLoad.toString());

                    if(classToLoad.isAnnotationPresent(PluginAnnotation.class)) {
                        for(Annotation annotation : classToLoad.getAnnotations()) {
                            if(annotation.annotationType().equals(PluginAnnotation.class)) {
                                PluginAnnotation pluginAnnotation = (PluginAnnotation) annotation;
                                classes.put(pluginAnnotation.name(), classToLoad);
                            }
                        }
                    }
                }
            }
        }
    }

    public static PluginXML readPluginXMLFile(File file) throws IOException, SAXException {
        XmlMapper xmlMapper = new XmlMapper();
        PluginXML pluginXML = xmlMapper.readValue(file, PluginXML.class);
        return pluginXML;
    }



}
