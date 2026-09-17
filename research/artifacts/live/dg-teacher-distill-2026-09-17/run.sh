#!/bin/bash
# Borrow VRAM on the 96 GB card from the OCR shard for the bench, and always hand it back.
cd ~/llmsort-dg
restore() { sudo systemctl start academic-ocr-gpu3; echo "OCR restarted $(date -u +%FT%TZ)" >> run.log; touch done.flag; }
trap restore EXIT
rm -f done.flag
sudo systemctl stop academic-ocr-gpu3
for i in $(seq 1 30); do free=$(nvidia-smi -i 3 --query-gpu=memory.free --format=csv,noheader,nounits); [ "$free" -gt 33000 ] && break; sleep 2; done
echo "free MiB on card: $free $(date -u +%FT%TZ)" >> run.log
CUDA_VISIBLE_DEVICES=GPU-d585f136-5660-6f59-4035-7fd58ed6bfc4 HF_HUB_OFFLINE=1 /srv/build/lanced-slice/venv-gpu/bin/python ${DG_SCRIPT:-dg_setwise.py} ${DG_ARGS:-smoke} >> run.log 2>&1
echo "exit $? $(date -u +%FT%TZ)" >> run.log
