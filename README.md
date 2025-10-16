# nlab-listary-demo
### Develop environment:
Ubuntu 22.04  

Insure you have set the Tauri and Dioxus platform in your system, then run  
```
cargo tauri dev
```
to build and examine the project  
The initialization process would take a long time.

---
### Project Structure(so far)：
frontend frame: Dioxus  
```
    ./nlab-listary/src  
    ├── app.rs  
    └── main.rs  
```
backend frame: tauri  
```
    ./nlab-listary/src-tauri/src  
    ├── browser.rs  
    ├── git_ops.rs  
    ├── lib.rs  
    ├── main.rs  
    ├── models.rs  
    ├── parser.rs  
    ├── search.rs  
    └── storage.rs  
```
The git repo named **nlab_mirror** will be saved in *./nlab-listary/src-tauri*.  
The database and search index are in the same diretory.  

---

### Backend Modules included are:  
Key data structures are in *models.rs*.

Git fetch nlab html pages from the official remote repository to local repository using **git2** crate in *git_ops.rs*.  
The backend syncronizing should be implemented in the future.

Html parsing using **walkdir** and **scraper** crate in *parser.rs*.  

Database using **sled** crate in *storage.rs*.

Searchengine using **tantivy** crate in *search.rs*.  

Entering the page by the url found opening the browser is implemented in *browser.rs*, using the **open** crate.  
But this part haven't got integrated.

*main.rs* integrated and verified the functions above.  

---
