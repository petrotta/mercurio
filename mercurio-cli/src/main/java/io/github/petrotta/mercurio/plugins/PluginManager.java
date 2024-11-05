package io.github.petrotta.mercurio.plugins;

import com.google.common.reflect.ClassPath;
import io.github.petrotta.mercurio.Application;



import java.io.File;
import java.io.IOException;

import java.net.URL;
import java.net.URLClassLoader;




public class PluginManager {

    public void index() throws Exception {
        File dir = Application.getPluginsDir();

        for (File file : dir.listFiles()) {
            System.out.println(file.getName());
            if (!file.isDirectory() && file.getName().endsWith(".jar")) {
                loadJar(file);
            }
        }

    }

    public void loadJar(File jar) throws IOException {

        URLClassLoader child = new URLClassLoader(new URL[] {jar.toURI().toURL()});
               // this.getClass().getClassLoader()
       // );


        ClassPath cp= ClassPath.from(child);
        for(ClassPath.ClassInfo info : cp.getAllClasses()   ) {
            //if(info.getClass().getClassLoader() == child)
             //System.out.print(info.getPackageName() + " - " + info.getSimpleName());
             //System.out.println(info.getClass().getClassLoader());

             try {
                 Class clazz = info.load();
                 if (clazz.getInterfaces().length > 0) {
                     System.out.println(clazz.getCanonicalName());
                 }
             } catch (RuntimeException e) {}


        }

//        Set<String> allClasses =
//                reflections.get(new Anno);

        //Class classToLoad = Class.forName("io.github.petrotta.mercurio.MyPlugin", true, child);

//      //  System.out.println(classToLoad);
////

////
////        Store store = reflections.getStore();
//
//        System.out.println( reflections.getAllTypes() );
//        //for(Class<?> module : modules) {
//        //    System.out.println(module.getName());
//        //}


    }
}
