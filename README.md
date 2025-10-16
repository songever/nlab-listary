# nlab-listary-demo
### Develop environment:
Ubuntu 22.04  

make sure you have set the Tauri and Dioxus platform in your system, then run  
```
cargo tauri dev
```
to build and examine the project.  
If you have something lacked please check https://tauri.app/start/prerequisites/.  

To open the browser in wsl, install wslu:  
```
apt install -y wslu
```

#### A brief description
The local index syncronize the data by cloning the official repo storing (maybe)all of the pages of the nlab-wiki.  
The first running process would take a long time, you should wait a few minutes for cloning the git repo.  
And you should prepare about 2GB spaces for the git repo.  

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
