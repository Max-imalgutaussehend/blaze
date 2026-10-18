document.querySelectorAll('[data-copy]').forEach(b=>b.addEventListener('click',()=>{
  const t=document.getElementById(b.dataset.copy).textContent;
  navigator.clipboard?.writeText(t).then(()=>{const o=b.textContent;b.textContent='Copied';setTimeout(()=>b.textContent=o,1200)});
}));
document.querySelectorAll('.tabs button').forEach(b=>b.addEventListener('click',()=>{
  document.querySelectorAll('.tabs button').forEach(x=>x.classList.toggle('on',x===b));
  const c=document.getElementById('cmd');c.textContent=b.dataset.cmd;
}));
