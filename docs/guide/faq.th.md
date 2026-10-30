# GitNova — FAQ และคู่มือ

[English](faq.en.md) · [ไทย](faq.th.md)

เปิด **Settings → FAQ** เพื่ออ่านและค้นหาคู่มือภายใน GitNova

## เริ่มต้น

### เปลี่ยนโฟลเดอร์โปรเจกต์ให้เป็น repository อย่างไร?

เปิดตัวเลือก workspace → Initialize repository… ใส่ path เต็มของโฟลเดอร์ใหม่หรือโฟลเดอร์ที่มีอยู่แล้วและ branch เริ่มต้น (ค่าเริ่มต้น `main`) ตรวจข้อมูลแล้วกด Initialize & open graph โฟลเดอร์แม่ต้องมีอยู่ก่อน ไฟล์เดิมไม่เปลี่ยนและยังไม่ถูก stage แอปไม่สร้าง commit แรกหรือ remote ให้อัตโนมัติ ถ้าเป็น Git repository อยู่แล้วให้ใช้ Open ดูขั้นตอนใน [คู่มือเริ่มต้น](../../README.md#start-a-local-repository)

### สร้าง commit แรกอย่างไร?

เปิด Working changes เลือกไฟล์เพื่อดู diff แล้วกด Stage file หรือ Stage all changes ใส่ข้อความสรุป ตรวจรายการที่ stage แล้วจึงยืนยัน commit เฉพาะเนื้อหาที่ stage จะเข้า commit การ commit ยังไม่เผยแพร่ ต้อง Push เพื่อส่งไป remote

### Workspace, repository และ worktree ต่างกันอย่างไร?

Workspace รวมโปรเจกต์เพื่อสลับทำงาน ไม่ใช่ Git submodule ส่วน repository เก็บประวัติ Git และ worktree คือพื้นที่ checkout ที่มีไฟล์ HEAD และ staging ของตนเอง Linked worktree ใช้ประวัติ repository ร่วมกัน สลับ repo จากตัวเลือก workspace และใช้ช่องค้นหาหรือแบ่งหน้าเมื่อมีหลายโปรเจกต์

## ใช้งานประจำวัน

### อ่านกราฟอย่างไร?

แต่ละจุดคือ commit เส้นเชื่อมไปยัง parent ป้าย branch และ tag ชี้ไปยัง commit ส่วน HEAD คือ checkout ปัจจุบัน Merge commit มีหลาย parent แต่ fast-forward แค่เลื่อนป้าย branch โดยไม่สร้างจุด merge เลือกแถวเพื่อดูไฟล์และ diff หรือคลิกขวาเพื่อเลือก action

### ปรับความกว้างคอลัมน์กราฟได้ไหม?

ลากเส้นแบ่งที่หัวคอลัมน์ ความกว้างจะบันทึกไว้ในเครื่อง ดับเบิลคลิกเส้นแบ่งหรือกด Home เมื่อโฟกัสอยู่ที่เส้นแบ่งเพื่อคืนค่าคอลัมน์นั้น ใช้ปุ่มลูกศรปรับได้ และกด Shift ร่วมเพื่อขยับมากขึ้น คอลัมน์ Graph มีความกว้างขั้นต่ำเพื่อให้เห็นเส้นกราฟครบ ถ้าประวัติมีหลายเส้นสามารถเลื่อนแนวนอนได้

### สลับ branch หรือเปิด remote branch อย่างไร?

ใช้ตัวเลือก branch หรือดับเบิลคลิกจุดกราฟหรือป้าย branch เพื่อเข้าสู่การตรวจ checkout ตรวจปลายทางก่อนยืนยัน การเปิด remote branch สามารถสร้าง local branch ที่ติดตาม remote ได้ การสลับ branch ของ parent repo จะไม่เปลี่ยน checkout ของ submodule โดยอัตโนมัติ

### Stage, Unstage และ Discard ต่างกันอย่างไร?

Stage นำการเปลี่ยนแปลงที่เลือกเข้า staging สำหรับ commit ถัดไป Unstage เอาออกจาก staging แต่เก็บไฟล์ที่แก้ไว้ ส่วน Discard คืนค่าตามขอบเขตที่ตรวจและอาจลบไฟล์ใหม่ ต้องตรวจขอบเขตก่อนยืนยัน หลัง operation ที่รองรับจะมีข้อมูล recovery ให้

### Stage แค่บางส่วนของไฟล์ได้ไหม?

เลือกไฟล์ข้อความปกติที่ Git ติดตามใน Working changes แล้วกด Stage hunk หรือ Unstage hunk ที่หัวบล็อก diff เพื่อแก้เฉพาะบล็อกนั้นใน index โดยเก็บไฟล์ทำงานไว้ ใช้ action รายไฟล์สำหรับไฟล์ใหม่ ไฟล์ที่ลบ เปลี่ยน mode, binary, เนื้อหาที่ไม่ใช่ UTF-8 หรือ patch ที่แสดงไม่ครบ ระบบจะปฏิเสธ preview ที่เก่าแล้ว

### Fetch, Pull, Push และ Refresh ทำอะไร?

Fetch รับประวัติจาก remote โดยยังไม่ merge เข้า local branch ส่วน Pull รับข้อมูลและรวมเข้ากับ branch ปัจจุบันตามวิธีที่ตรวจ Push ส่งประวัติ local ที่เลือกไป remote ขณะที่ Refresh อ่านสถานะ Git ในเครื่องใหม่ ไม่ได้แทน Fetch บน macOS การเชื่อมต่อ HTTPS ใช้บัญชี host ของ GitNova ที่ตรงกันถ้ามี มิฉะนั้นใช้ credentials ของ Git ส่วน SSH ใช้ agent เดิมของคุณ

### ตัวเลขลูกศรขึ้นและลงคืออะไร?

↑ คือจำนวน commit ใน local ที่ remote branch ที่ติดตามยังไม่มี ส่วน ↓ คือ commit ฝั่ง remote ที่ local ยังไม่มี ถ้าประวัติแยกกันอาจมีทั้งสองค่า ตัวเลขอิงข้อมูล remote ที่ Fetch ล่าสุด ไม่ใช่สถานะสดบน server ควร Fetch ก่อนตัดสินใจรวมงาน

### สืบค้นประวัติไฟล์หรือโค้ดอย่างไร?

เปิด Search commits แล้วเลือก File / code history ใส่ path ใน repo แบบตรงตัว ข้อความโค้ด หรือทั้งสองอย่าง เลือกขอบเขต repo แล้วกด Search การค้นโค้ดหาจุดที่จำนวนข้อความเปลี่ยน ไม่ใช่ทุกครั้งที่แก้บรรทัดนั้น ตรวจ commit ล่าสุดไม่เกิน 2,000 รายการต่อพื้นที่ และแสดงผลไม่เกิน 100 รายการ Merge เทียบกับ parent แรก และไม่ตามการเปลี่ยนชื่อไฟล์ เลือกผลเพื่อเปิดกราฟและ diff ในอดีตโดยไม่ checkout หากประวัติเป็น shallow หรือถึงขีดจำกัดจะมีข้อความแจ้ง

## ประวัติและการกู้คืน

### Stash, Apply และ Pop ใช้อย่างไร?

Stash เก็บงานที่ยังไม่เสร็จ Apply นำกลับมาที่ branch ที่ checkout อยู่และเก็บ stash ไว้ Pop นำกลับมาที่ branch ปัจจุบันแล้วเอา stash ออกเมื่อสำเร็จ เช่น stash ที่ main สลับไป develop แล้ว pop งานจะมาที่ develop ถ้า conflict จะยังเก็บ stash ไว้ ต้องจัดการ Git operation ที่ค้างก่อนคืน stash อื่น

### Merge กับ Fast-forward ต่างกันอย่างไร?

Merge รวมประวัติเข้า branch ปัจจุบัน Create merge commit เก็บปลายทั้งสอง branch เป็น parent จึงเห็นจุดรวมในกราฟ Fast-forward ใช้ได้เมื่อปลายปัจจุบันเป็นบรรพบุรุษของปลายที่จะนำเข้า โดยเลื่อน branch อย่างเดียว ตัวเลือกลบ source จะลบเฉพาะ local branch ที่เข้าเงื่อนไขหลังตรวจว่า merge สำเร็จแล้ว

### Cherry-pick คืออะไร ทำไมต้องเลือก parent?

Cherry-pick นำการเปลี่ยนแปลงของ commit หนึ่งมาใช้บน branch ปัจจุบันเป็น commit ใหม่ Merge commit มีหลาย parent จึงต้องเลือกฐานในการคำนวณสิ่งที่เปลี่ยน Parent 1 มักเป็น branch ปลายทางก่อน merge ให้ตรวจ preview ก่อนเลือก ส่วน Parent 2 จะได้ชุดการเปลี่ยนแปลงที่ต่างกัน

### Rebase คืออะไร?

Rebase นำ commit ไปเล่นซ้ำบนฐานใหม่และเปลี่ยนรหัส คลิกขวา commit ปลายทาง เลือก Rebase ตรวจ branch ปัจจุบัน → ปลายทางและช่วง commit แล้วยืนยันการเขียนใหม่ รองรับช่วงเส้นตรงไม่เกิน 100 commit โดยปฏิเสธช่วง merge/root และการเปลี่ยน pointer ของ submodule แก้ conflict ใน Working changes แล้วตรวจ Continue, Skip empty commit หรือ Abort ปลายเดิมยังใช้กู้คืนได้ และไม่ force push

### Amend กับ Revert คืออะไร?

Amend แทนที่ commit ล่าสุดด้วยข้อความและเนื้อหา staged ที่ตรวจแล้ว ทำให้รหัสเปลี่ยน ไม่ได้รวม commit เก่าใด ๆ เข้าด้วยกัน ส่วน Revert สร้าง commit ใหม่เพื่อย้อนการเปลี่ยนแปลงของ commit ที่เลือก เหมาะเมื่ออยากรักษาประวัติที่แชร์กับทีม

### แก้ conflict อย่างไร?

เปิด Working changes แล้วเลือก conflict เทียบ Current กับ Incoming เลือกทีละบล็อกข้อความหรือแก้ Result จากนั้นตรวจและ stage ผลที่แก้ ทำต่อหรือ commit operation เมื่อแก้ครบทุก conflict แล้ว Current/Incoming ให้อิง operation ที่แสดง โดยเฉพาะตอน rebase ส่วน binary และ submodule ต้องใช้ตัวเลือกเฉพาะ

## หลาย repository

### Submodule คืออะไร?

Submodule เป็น Git repository แยกที่ parent บันทึกรหัส commit ไว้ แต่ละ branch ของ parent บันทึกคนละ commit ได้ ค่า recorded, staged และ checkout อาจต่างกัน Dirty หมายถึงไฟล์ภายในมีการแก้ ส่วน Commit changed คือ checkout ไม่ตรง pointer ที่บันทึก Detached HEAD เป็นเรื่องปกติเมื่อ checkout commit ที่ parent ระบุ

### ควร Restore submodule หรือ Stage pointer?

ใช้แถว Submodules ของ branch ที่เลือกและ Manage เพื่อตรวจสถานะ เปิดกราฟของ child เพื่อจัดการไฟล์ Restore recorded คืน checkout ไป commit ที่ parent บันทึกไว้ ส่วน Stage pointer ให้ parent บันทึก commit ที่ checkout อยู่ ตรวจงานค้างและ commit ที่ยังไม่ push ก่อน Sync URLs นำ URL จาก .gitmodules ที่ตรวจแล้วไปใส่ config ของ child ที่ initialize แล้ว โดยไม่ fetch ไม่แก้ไฟล์ และไม่เปลี่ยน commit สำหรับรูปแบบ URL มาตรฐานที่รองรับ จะคำนวณ relative URL จาก remote ของ branch ใน parent ต้องตรวจปลายทางที่คำนวณแล้วก่อนยืนยัน

### ควรใช้ linked worktree เมื่อไร?

ใช้ linked worktree เพื่อมีอีก checkout ของ repository เดิม เช่น ตรวจ release โดยเก็บพื้นที่ทำ feature ไว้ เปิดหรือสร้างจากบริบทกราฟ ใน Related repositories เลือก Worktree metadata เพื่อตรวจและสั่ง Lock, Unlock หรือ Repair ของ linked checkout ที่มีอยู่ Lock ป้องกันการ prune ส่วน Unlock ยกเลิกการป้องกัน Repair ซ่อม metadata โดยไม่ลบไฟล์หรือย้าย branch ระบบไม่ลบ worktree ที่มีงานค้างให้อัตโนมัติ

### เปรียบเทียบ release branch ข้าม repo ได้ไหม?

ใน Search commits เลือก Compare branches across repositories และขอบเขต repo ปัจจุบัน ที่เลือก หรือทั้ง workspace ใส่ Source และ Target branch แล้วกด Search จะแสดง commit เฉพาะแต่ละฝั่ง SHA ปลาย branch และจำนวนไฟล์ต่างกันต่อพื้นที่ หากไม่มี ref ประวัติเป็น shallow หรือถึงขีดจำกัดจะแจ้งให้เห็น การเปรียบเทียบไม่ fetch remote ให้อัตโนมัติ เป็นหลักฐานจาก Git ที่ commit แล้ว ไม่รับรอง CI ความเข้ากันได้ของ dependency หรือความพร้อม release

## แชร์งานและการเชื่อมต่อ

### Clone หรือเผยแพร่ repo ใหม่อย่างไร?

Clone จากตัวเลือก workspace ไปโฟลเดอร์ใหม่ ถ้าจะเผยแพร่งานในเครื่องให้ commit ก่อนแล้ว Push หากไม่มี remote ให้ Add remote เพื่อใส่ URL repo ที่มีอยู่ หรือสร้าง repo ว่างบน GitHub/GitLab ผ่าน Connection ส่วนกลาง การสร้าง repo ตั้ง remote ในเครื่อง และ Push เป็นคนละขั้นที่ต้องตรวจ เลือกสิทธิ์การมองเห็นให้ถูกต้อง

### สร้างและ push tag version อย่างไร?

คลิกขวา commit ที่ต้องการแล้วเลือก Create tag ตรวจชื่อและ commit ให้ตรง Tag ในเครื่องไม่ได้ส่งออกด้วยการ push branch ตามปกติ ต้องใช้ Push ของ tag และตรวจปลายทาง ระบบจะไม่เขียนทับ tag บน remote โดยเงียบ ๆ

### เปิด pull request หรือ merge request อย่างไร?

ใช้ action ขอรวมงานของ branch และตรวจทิศทาง source → target ต้องเผยแพร่ source branch ก่อน GitHub เรียก pull request ส่วน GitLab เรียก merge request Connection ต้องมีสิทธิ์ API ที่เกี่ยวข้อง การสร้าง request ยังไม่ได้ merge branch

### Copy commit link คัดลอกอะไร?

คัดลอก URL เว็บบน remote ของ commit ที่เลือกโดยใช้ SHA เต็ม GitNova ใช้ fetch upstream ของ branch ปัจจุบันก่อน ตามด้วย origin หรือ remote เดียวที่ตั้งไว้ Connections ส่วนกลางระบุชนิด Git host ภายใน การสร้างลิงก์ไม่ต้องใช้ token แต่ commit ที่ยังไม่ push อาจเปิดไม่ได้สำหรับทีม และลิงก์ไม่ได้ให้สิทธิ์เข้าถึงเพิ่ม

### ตั้งบัญชีและ CI ที่ไหน?

เปิด Settings → Connections บัญชีใช้ส่วนกลางตาม Git host ไม่ได้ผูกกับ repo เดียว สถานะ CI อยู่ระหว่าง Commit message กับ Author เมื่อตั้งค่าและมีข้อมูล GitLab pipeline และข้อมูล Jenkins ที่ตั้งไว้ต้องอ่านจากบริการ ไม่ได้มาจาก Git ในเครื่องอย่างเดียว บน macOS บัญชี GitHub/GitLab ที่บันทึกไว้ใช้ยืนยันตัวตนกับ HTTPS Git ได้ด้วย โดยอ่าน token จาก Keychain ผ่าน helper เฉพาะ process ถ้าไม่มีบัญชีในแอปจะใช้ credentials เดิมของ Git ส่วน SSH ใช้ agent สิทธิ์อ่านและเขียนยังขึ้นอยู่กับ token

## ประวัติและการกู้คืน

### ถ้า operation ล้มเหลวหรือค้างควรทำอย่างไร?

เปิด Working changes เพื่อตรวจ merge, cherry-pick, revert หรือ rebase ที่เริ่มใน GitNova ซึ่งยังค้าง แก้ conflict แล้วตรวจ commit/Continue หรือ Abort หาก journal หลังการขัดจังหวะไม่แน่นอนต้องตรวจเพิ่มเติม แอปไม่ลองซ้ำเอง Rebase ที่เริ่มภายนอกต้องจัดการผ่าน Git ภายนอกแอป จำนวนไฟล์เป็นศูนย์ไม่ได้แปลว่าไม่มี operation ค้าง ดู Recent activity and recovery เพื่อหารหัสอ้างอิงหรือที่สำรองไว้และตรวจก่อนกู้คืน

### ทำไมผู้เขียน commit ถึงต่างจากบัญชีที่เชื่อมต่อ?

Connection ใช้ยืนยันสิทธิ์ Git, PR/MR และ CI ส่วนชื่อผู้เขียน commit ปกติมาจาก
Git config กำหนดชื่อและอีเมลแยกตามโฮสต์ได้ที่ **Settings → Connections →
Commit identity** สำหรับ GitHub ให้ใช้อีเมลที่ยืนยันแล้วหรือ GitHub noreply
ระบบจะไม่เดาอีเมลจากชื่อบัญชี

GitNova จับคู่ `origin` หรือ remote เดียวเมื่อไม่มี `origin` หากมีหลาย remote
หรือจับคู่โฮสต์ได้กำกวม จะให้ตรวจสอบแทนการเลือกบัญชีเอง หากไม่มีตัวตนที่ตรงกัน
จะใช้ Git config ตามเดิม โดยไม่แก้ไฟล์ config ของผู้ใช้ ตรวจบรรทัด **Committer**
ก่อนยืนยันเสมอ ส่วน amend, cherry-pick และ rebase รักษาผู้เขียนเดิมไว้ การบันทึก
ตัวตนไม่แก้ commit หรือประวัติที่เผยแพร่แล้ว หากเปลี่ยน identity หรือ remote
หลัง preview ต้องตรวจใหม่อีกครั้ง ควรทำ merge/rebase ที่ค้างให้เสร็จก่อนเปลี่ยนตัวตน

## เปิดเทอร์มินัลในที่เก็บโค้ดปัจจุบันอย่างไร?

กด Terminal ข้าง Pull/Push เพื่อเปิด dock ใต้กราฟ โดยเริ่มเชลล์ที่โฟลเดอร์ของ
repo หรือ linked worktree ปัจจุบัน แต่ละรายการมีเซสชันของตัวเอง การสลับ repo
ไม่ส่งคำสั่งไปยังเชลล์ของ repo ก่อนหน้า ปุ่มซ่อนจะเก็บเชลล์ให้ทำงานต่อ ส่วน
End session จะถามก่อนปิด เปิดค้างได้สูงสุด 4 เซสชัน การปิดหน้าต่างหรือโหลดแอปใหม่
จะปิดเซสชัน โปรแกรมที่แยกไปทำงานเบื้องหลังอาจมีอายุการทำงานของตัวเอง
ชื่อโฟลเดอร์ที่แสดงคือจุดเริ่มต้น แม้จะใช้ `cd` ไปที่อื่นแล้วก็ตาม

คำสั่งทำงานโดยตรงตามการตั้งค่าเชลล์และ Git ปกติ ไม่ผ่าน preview หรือตัวตนผู้เขียน
ที่ตั้งใน connection ของ GitNova ใช้ Refresh เพื่อดูการเปลี่ยนแปลงจากคำสั่งภายนอก
Auto fetch จะพักระหว่างมีเซสชันเปิดอยู่ จำกัดประวัติ output เพื่อคุมหน่วยความจำ
GitNova ไม่บันทึก transcript แต่เชลล์อาจเก็บประวัติตามการตั้งค่าปกติของคุณ
